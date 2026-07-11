import JSZip from 'jszip';
import {
  processImageWasm,
  cellToDataUrl,
  pathsToSvgDataUrl,
  getWasmBuildInfo,
} from '../wasm/loader';
import type { WasmProcessedCell, WasmCellQuality } from '../wasm/loader';
import { getCharactersForPage, flagToSelection } from '../../data/characters';
import type { VectorGlyph } from '../font/builder';

export interface ProcessMessage {
  type: 'info' | 'warning' | 'error' | 'success';
  text: string;
}

export interface GlyphStatus {
  char: string;
  unicode: number;
  pageIndex: number;
  row: number;
  col: number;
  status: 'found' | 'empty' | 'imported';
  cellImageDataUrl?: string; // セル切り出し画像のData URL
  /**
   * 要確認フラグ（#110: セル品質ゲート）。
   * 採用セルで境界接触残渣の除去・はみ出しストローク保護などが発生した場合に真。
   * review UI で「要確認」マークを付けてユーザーに目視確認を促す（黙って空に倒さない）。
   */
  needsReview?: boolean;
}

export interface ProcessCallbacks {
  onPageStart: (page: number, total: number) => void;
  onMessage: (msg: ProcessMessage) => void;
  onPageCorrected?: (pageIndex: number, canvas: HTMLCanvasElement) => void;
  onGlyphStatus?: (status: GlyphStatus) => void;
}

export interface ProcessResult {
  glyphs: VectorGlyph[];
}

/**
 * 補正後画像（RGBA配列）からCanvasを生成する
 */
function correctedImageToCanvas(
  imageData: number[],
  width: number,
  height: number,
): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d')!;
  const data = new Uint8ClampedArray(imageData);
  const imgData = new ImageData(data, width, height);
  ctx.putImageData(imgData, 0, 0);
  return canvas;
}

/**
 * 段階診断ログ（#109）。
 * スキャンパイプラインのどの段階まで到達したか／どこで落ちたかを
 * console から判別できるようにする。UI の挙動には影響しない。
 * WASM 内部の詳細ログ（`=== ステップN ===`）と併せて読む。
 */
function scanLog(stage: string, message: string, failed = false): void {
  const line = `[scan:${stage}] ${message}`;
  if (failed) {
    console.error(line);
  } else {
    console.info(line);
  }
}

/**
 * WASM エラーメッセージから、パイプラインのどの段階で失敗したかを推定する。
 * Rust 側のエラー文言（marker.rs / pipeline.rs）に依存する。
 */
export function inferFailedStage(rawError: string): string {
  if (rawError.includes('マーカー')) return 'marker';
  if (rawError.includes('解像度') || rawError.includes('DPI')) return 'dpi';
  if (rawError.includes('歪み') || rawError.includes('撮り直し')) return 'perspective';
  if (
    rawError.includes('画像') &&
    (rawError.includes('デコード') || rawError.includes('フォーマット'))
  )
    return 'decode';
  return 'wasm';
}

/**
 * WASMからのエラーメッセージをユーザーフレンドリーな日本語に変換する。
 * `buildSha` が渡された場合はエラー末尾に `[build: sha]` を付加し、報告時に版特定を容易にする。
 * 省略時は純粋な変換のみ行う（テスト容易性のため副作用なし）。
 */
export function translateWasmError(rawError: string, buildSha?: string | null): string {
  let msg: string;
  // マーカー検出失敗
  if (rawError.includes('マーカーが検出できませんでした')) {
    msg =
      '用紙のマーカーを検出できませんでした。紙全体が写るよう、なるべく正面から撮影してください。';
  } else {
    // DPI不足: Rust側のメッセージにDPI値と推奨値が含まれているのでそのまま通す
    // その他のRustエラーもそのまま返す
    msg = rawError;
  }
  if (buildSha) {
    msg += ` [build: ${buildSha}]`;
  }
  return msg;
}

/**
 * 1文字分のセル群から要確認フラグを集約する（#110）。
 * 採用セルのどれかで品質ゲートが発動していたら true。
 * 採用されていないセルの needs_review はグリフに影響しないため無視する。
 * quality 欠落（stale wasm）時はクラッシュせず false に倒す
 * （欠落自体の検知・警告は warnIfQualityMissing が担う）。
 */
export function glyphNeedsReview(
  cells: { adopted: boolean; quality?: WasmCellQuality | null }[],
): boolean {
  return cells.some((c) => c.adopted && c.quality?.needs_review === true);
}

/**
 * stale wasm ガード（#110）: wasm 出力の quality フィールド欠落を検出する。
 * Rust 側の ProcessedCell に quality が増えた後、古い wasm ビルドのまま動くと
 * フィールドが黙って落ちて needs_review が常に false になる（偽緑）。
 * 黙って false に倒さず、[scan:cells] の error ログで気づけるようにする。
 * 欠落があれば true を返す。
 */
export function warnIfQualityMissing(
  cells: { quality?: WasmCellQuality | null }[],
  pageLabel: string,
): boolean {
  const missing = cells.filter((c) => c.quality == null).length;
  if (missing === 0) return false;
  scanLog(
    'cells',
    `${pageLabel} ${missing} セルで quality フィールドが欠落しています（古い wasm ビルドの可能性。npm run wasm:build を実行してください）`,
    true,
  );
  return true;
}

// メイン処理
export async function processImages(
  files: File[],
  callbacks: ProcessCallbacks,
): Promise<ProcessResult> {
  // Issue #93: 同一 unicode が複数ページ／複数画像で検出されたときの重複を防ぐため、
  // ベースグリフは Map で後勝ち上書き、alt-variant は別配列に積んで関数末尾で結合する。
  const baseGlyphs = new Map<number, VectorGlyph>();
  const altGlyphs: VectorGlyph[] = [];

  // ZIP展開 + 画像ファイル収集
  let imageFiles: File[] = [];
  for (const file of files) {
    if (file.name.endsWith('.zip') || file.type === 'application/zip') {
      const zip = await JSZip.loadAsync(file);
      for (const [name, entry] of Object.entries(zip.files)) {
        if (entry.dir) continue;
        if (name.startsWith('__MACOSX') || name.includes('/._')) continue;
        const ext = name.toLowerCase().split('.').pop();
        if (['jpg', 'jpeg', 'png', 'webp'].includes(ext || '')) {
          const blob = await entry.async('blob');
          const imgFile = new File([blob], name, { type: `image/${ext === 'jpg' ? 'jpeg' : ext}` });
          imageFiles.push(imgFile);
        }
      }
    } else if (file.type.startsWith('image/')) {
      imageFiles.push(file);
    }
  }

  if (imageFiles.length === 0) {
    callbacks.onMessage({ type: 'error', text: '画像ファイルが見つかりませんでした。' });
    return { glyphs: [] };
  }

  callbacks.onPageStart(0, imageFiles.length);

  for (let fi = 0; fi < imageFiles.length; fi++) {
    callbacks.onPageStart(fi + 1, imageFiles.length);
    const fileName = imageFiles[fi].name;
    scanLog('pipeline', `file="${fileName}" (${fi + 1}/${imageFiles.length}) 開始`);

    let wasmResult;
    try {
      wasmResult = await processImageWasm(imageFiles[fi]);
    } catch (e) {
      const rawError = e instanceof Error ? e.message : String(e);
      scanLog(inferFailedStage(rawError), `file="${fileName}" 失敗: ${rawError}`, true);
      const msg = translateWasmError(rawError, getWasmBuildInfo()?.sha);
      callbacks.onMessage({
        type: 'error',
        text: `ファイル "${imageFiles[fi].name}" の処理に失敗しました: ${msg}`,
      });
      continue;
    }

    // WASM 成功 = マーカー検出・台形補正・セル切り出し・二値化・ベクター化は完走している
    scanLog(
      'perspective',
      `file="${fileName}" ok corrected=${wasmResult.corrected_width}x${wasmResult.corrected_height}`,
    );

    const pageNumber = wasmResult.page_number;
    if (!pageNumber) {
      scanLog('qr', `file="${fileName}" 失敗: QR からページ番号を取得できない`, true);
      callbacks.onMessage({
        type: 'error',
        text: `画像 ${fi + 1} のQRコードを読み取れませんでした。画像が不鮮明な可能性があります。`,
      });
      continue;
    }
    scanLog('qr', `file="${fileName}" ok page=${pageNumber}/${wasmResult.total_pages ?? '?'}`);

    // Issue #96: リトライ用 PDF は QR に文字リスト (`chars`) を直接埋め込むため、
    // それを最優先で使う。CharSelection に当てはまらない任意文字リストでも復元できる。
    // chars が無いときは Issue #91 の従来パス: `s` フラグから文字セット選択を復元する。
    let pageChars: string[];
    if (wasmResult.qr_chars && wasmResult.qr_chars.length > 0) {
      pageChars = wasmResult.qr_chars;
      scanLog('chars', `page=${pageNumber} ok count=${pageChars.length} source=qr_chars`);
    } else {
      const selectionFlag = wasmResult.char_selection;
      const selection = selectionFlag ? flagToSelection(selectionFlag) : null;
      if (!selection) {
        scanLog(
          'chars',
          `page=${pageNumber} 失敗: 文字セット選択フラグを復元できない (s=${selectionFlag ?? 'null'})`,
          true,
        );
        callbacks.onMessage({
          type: 'error',
          text: `画像 ${fi + 1} のQRから文字セット情報を取得できませんでした。古い版のテンプレートの可能性があります。PDF を再生成して印刷してください。`,
        });
        continue;
      }
      pageChars = getCharactersForPage(pageNumber - 1, selection);
      scanLog(
        'chars',
        `page=${pageNumber} ok count=${pageChars.length} source=selection(${selectionFlag})`,
      );
    }

    // 補正後キャンバスをコールバックで通知
    if (callbacks.onPageCorrected) {
      const correctedCanvas = correctedImageToCanvas(
        wasmResult.corrected_image,
        wasmResult.corrected_width,
        wasmResult.corrected_height,
      );
      callbacks.onPageCorrected(pageNumber, correctedCanvas);
    }

    // セルを (row, col) でグループ化
    const cellsByPos = new Map<string, WasmProcessedCell[]>();
    for (const cell of wasmResult.cells) {
      const key = `${cell.row},${cell.col}`;
      if (!cellsByPos.has(key)) cellsByPos.set(key, []);
      cellsByPos.get(key)!.push(cell);
    }

    // stale wasm ガード（#110）: quality 欠落は error ログで警告（黙って false に倒さない）
    warnIfQualityMissing(wasmResult.cells, `page=${pageNumber}`);

    const totalCells = wasmResult.cells.length;
    const adoptedCellCount = wasmResult.cells.filter((c) => c.adopted).length;
    const emptyCellCount = wasmResult.cells.filter((c) => c.is_empty).length;
    // 品質ゲート（#110）で要確認になった採用セル数。採用されていないセルの
    // 残渣除去はグリフに影響しないため、採用セルだけを数える
    const reviewCellCount = wasmResult.cells.filter(
      (c) => c.adopted && c.quality?.needs_review === true,
    ).length;
    scanLog(
      'cells',
      `page=${pageNumber} ok cells=${totalCells} adopted=${adoptedCellCount} empty=${emptyCellCount} review=${reviewCellCount}`,
    );

    let pageGlyphCount = 0;
    let pageEmptyPathCount = 0; // 採用されたのにベクター化結果が空のセル数

    for (const [, cells] of cellsByPos) {
      const firstCell = cells[0];
      const charIndex = firstCell.char_index;
      if (charIndex === null || charIndex === undefined || charIndex >= pageChars.length) continue;

      const char = pageChars[charIndex];
      const unicode = char.codePointAt(0)!;

      // 採用されたセルを抽出
      const adoptedCells = cells.filter((c) => c.adopted);

      if (adoptedCells.length === 0) {
        callbacks.onGlyphStatus?.({
          char,
          unicode,
          pageIndex: pageNumber,
          row: firstCell.row,
          col: firstCell.col,
          status: 'empty',
        });
        continue;
      }

      // プレビュー用 Data URL（UI表示用、最初の採用セル）
      // TTF に近い見た目を示すためベジェパスの SVG を優先。paths が空ならラスタにフォールバック
      let cellImageDataUrl: string | undefined;
      const previewCell = adoptedCells[0];
      try {
        if (previewCell.paths && previewCell.paths.length > 0) {
          cellImageDataUrl = pathsToSvgDataUrl(previewCell.paths);
        } else {
          cellImageDataUrl = cellToDataUrl(previewCell);
        }
      } catch {
        /* non-browser environment */
      }

      callbacks.onGlyphStatus?.({
        char,
        unicode,
        pageIndex: pageNumber,
        row: firstCell.row,
        col: firstCell.col,
        status: 'found',
        cellImageDataUrl,
        // 採用セルのどれかで品質ゲート（#110）が発動していたら要確認
        needsReview: glyphNeedsReview(cells),
      });

      // 既に同 unicode のベースグリフが登録済みなら、対応する旧 alt-variant を破棄
      // （後勝ちで上書きされるベースに紐づく古い alt が残らないようにする）
      if (baseGlyphs.has(unicode)) {
        const hex = unicode.toString(16).toUpperCase().padStart(4, '0');
        const altPrefix = `uni${hex}.alt`;
        for (let i = altGlyphs.length - 1; i >= 0; i--) {
          if (altGlyphs[i].name.startsWith(altPrefix)) altGlyphs.splice(i, 1);
        }
      }

      pageGlyphCount++;

      for (let ai = 0; ai < adoptedCells.length; ai++) {
        const cell = adoptedCells[ai];
        // ベクター化は Rust 側で完結済み。WASM 出力の paths をそのまま使う
        const paths = cell.paths;
        if (paths.length === 0) {
          pageEmptyPathCount++;
          scanLog(
            'vectorize',
            `page=${pageNumber} char="${char}" (row=${cell.row},col=${cell.col}) 採用セルのパスが空`,
            true,
          );
        }

        const name =
          ai === 0
            ? `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}`
            : `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}.alt${ai}`;

        const glyph: VectorGlyph = {
          name,
          unicode: ai === 0 ? unicode : undefined,
          paths,
          advanceWidth: 1000,
        };

        if (ai === 0) {
          // ベースグリフは後勝ち
          baseGlyphs.set(unicode, glyph);
        } else {
          altGlyphs.push(glyph);
        }
      }
    }

    scanLog(
      'vectorize',
      `page=${pageNumber} ok glyphs=${pageGlyphCount} emptyPaths=${pageEmptyPathCount}`,
      pageEmptyPathCount > 0,
    );
  }

  scanLog('font-input', `ok base=${baseGlyphs.size} alt=${altGlyphs.length}`);
  return { glyphs: [...baseGlyphs.values(), ...altGlyphs] };
}
