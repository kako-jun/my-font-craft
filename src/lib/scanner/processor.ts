import JSZip from 'jszip';
import {
  processImageWasm,
  cellToDataUrl,
  pathsToSvgDataUrl,
  getWasmBuildInfo,
} from '../wasm/loader';
import type { WasmProcessedCell } from '../wasm/loader';
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

    let wasmResult;
    try {
      wasmResult = await processImageWasm(imageFiles[fi]);
    } catch (e) {
      const msg = translateWasmError(
        e instanceof Error ? e.message : String(e),
        getWasmBuildInfo()?.sha,
      );
      callbacks.onMessage({
        type: 'error',
        text: `ファイル "${imageFiles[fi].name}" の処理に失敗しました: ${msg}`,
      });
      continue;
    }

    const pageNumber = wasmResult.page_number;
    if (!pageNumber) {
      callbacks.onMessage({
        type: 'error',
        text: `画像 ${fi + 1} のQRコードを読み取れませんでした。画像が不鮮明な可能性があります。`,
      });
      continue;
    }

    // Issue #91: QR に含まれる文字セット選択フラグ (`s`) から、そのページが
    // どの文字集合を印刷したものかを復元する。取得できないときは古い版の
    // テンプレートの可能性が高いので、そのページはスキップする。
    const selectionFlag = wasmResult.char_selection;
    const selection = selectionFlag ? flagToSelection(selectionFlag) : null;
    if (!selection) {
      callbacks.onMessage({
        type: 'error',
        text: `画像 ${fi + 1} のQRから文字セット情報を取得できませんでした。古い版のテンプレートの可能性があります。PDF を再生成して印刷してください。`,
      });
      continue;
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

    // ページの文字リスト（選択された文字セットに限定）
    const pageChars = getCharactersForPage(pageNumber - 1, selection);

    // セルを (row, col) でグループ化
    const cellsByPos = new Map<string, WasmProcessedCell[]>();
    for (const cell of wasmResult.cells) {
      const key = `${cell.row},${cell.col}`;
      if (!cellsByPos.has(key)) cellsByPos.set(key, []);
      cellsByPos.get(key)!.push(cell);
    }

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

      for (let ai = 0; ai < adoptedCells.length; ai++) {
        const cell = adoptedCells[ai];
        // ベクター化は Rust 側で完結済み。WASM 出力の paths をそのまま使う
        const paths = cell.paths;

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
  }

  return { glyphs: [...baseGlyphs.values(), ...altGlyphs] };
}
