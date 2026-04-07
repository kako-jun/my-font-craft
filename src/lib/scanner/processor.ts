import JSZip from 'jszip';
import { readQRFromCanvas } from './qr-reader';
import {
  detectMarkers,
  extrapolatePageCorners,
  perspectiveTransform,
  detectOrientation,
  rotateCanvas,
} from './marker-detector';
import {
  mm,
  PAGE_WIDTH,
  PAGE_HEIGHT,
  COLS,
  CELL_SIZE,
  CHECK_HEIGHT,
  INNER_SIZE,
  CYAN_SAMPLE_X,
  CYAN_SAMPLE_Y,
  CYAN_SAMPLE_SIZE,
  GRAY_BAR_STEPS,
  GRAY_BAR_STEP_SIZE,
  GRAY_BAR_LEFT_X,
  GRAY_BAR_RIGHT_X,
  GRAY_BAR_TOP_Y,
  GRAY_BAR_BOTTOM_Y,
  getCellPosition,
} from '../template/layout';
import { getCharactersForPage } from '../../data/characters';
import type { VectorGlyph } from '../font/builder';
import { vectorizeGlyph } from '../vectorizer/contour';

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

// 画像ファイルからCanvasに読み込む
async function loadImageToCanvas(file: File): Promise<HTMLCanvasElement> {
  const bitmap = await createImageBitmap(file);
  const canvas = document.createElement('canvas');
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close();
  return canvas;
}

// シアン色の除去
function removeCyan(canvas: HTMLCanvasElement, cyanR: number, cyanG: number, cyanB: number) {
  const ctx = canvas.getContext('2d')!;
  const imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const data = imgData.data;
  const tolerance = 80; // 色距離の閾値

  for (let i = 0; i < data.length; i += 4) {
    const dr = data[i] - cyanR;
    const dg = data[i + 1] - cyanG;
    const db = data[i + 2] - cyanB;
    const dist = Math.sqrt(dr * dr + dg * dg + db * db);
    if (dist < tolerance) {
      data[i] = 255;
      data[i + 1] = 255;
      data[i + 2] = 255;
    }
  }

  ctx.putImageData(imgData, 0, 0);
}

// シアンサンプルの平均色を読み取る
function readCyanSample(canvas: HTMLCanvasElement): [number, number, number] {
  const ctx = canvas.getContext('2d')!;
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);
  const sx = Math.round(mm(CYAN_SAMPLE_X) * scaleX);
  const sy = Math.round(mm(CYAN_SAMPLE_Y) * scaleY);
  const sw = Math.round(mm(CYAN_SAMPLE_SIZE) * scaleX);
  const sh = Math.round(mm(CYAN_SAMPLE_SIZE) * scaleY);

  const data = ctx.getImageData(sx, sy, Math.max(1, sw), Math.max(1, sh)).data;
  let r = 0,
    g = 0,
    b = 0;
  const count = data.length / 4;
  for (let i = 0; i < data.length; i += 4) {
    r += data[i];
    g += data[i + 1];
    b += data[i + 2];
  }
  return [Math.round(r / count), Math.round(g / count), Math.round(b / count)];
}

// グレースケールバー読み取り結果
interface GrayBarReadings {
  left: number[]; // 左バーの各ステップの平均輝度 (0-255)
  right: number[]; // 右バーの各ステップの平均輝度 (0-255)
}

// 左右縦グレースケールバーを読み取る
function readGrayBars(canvas: HTMLCanvasElement): GrayBarReadings {
  const ctx = canvas.getContext('2d')!;
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);
  const barHeight = GRAY_BAR_BOTTOM_Y - GRAY_BAR_TOP_Y;
  const stepHeight = barHeight / GRAY_BAR_STEPS;

  function readBar(barX: number): number[] {
    const values: number[] = [];
    for (let i = 0; i < GRAY_BAR_STEPS; i++) {
      const y = GRAY_BAR_TOP_Y + i * stepHeight;
      const px = Math.round(mm(barX) * scaleX);
      const py = Math.round(mm(y) * scaleY);
      const pw = Math.round(mm(GRAY_BAR_STEP_SIZE) * scaleX);
      const ph = Math.round(mm(stepHeight) * scaleY);
      // 中央部分のみサンプリング（端のにじみを避ける）
      const marginPx = Math.max(1, Math.floor(Math.min(pw, ph) * 0.2));
      const sx = Math.max(0, Math.min(px + marginPx, canvas.width - 1));
      const sy = Math.max(0, Math.min(py + marginPx, canvas.height - 1));
      const sw = Math.max(1, Math.min(pw - marginPx * 2, canvas.width - sx));
      const sh = Math.max(1, Math.min(ph - marginPx * 2, canvas.height - sy));
      const data = ctx.getImageData(sx, sy, sw, sh).data;
      let sum = 0;
      const count = data.length / 4;
      for (let j = 0; j < data.length; j += 4) {
        sum += (data[j] + data[j + 1] + data[j + 2]) / 3;
      }
      values.push(count > 0 ? sum / count : 128);
    }
    return values;
  }

  return {
    left: readBar(GRAY_BAR_LEFT_X),
    right: readBar(GRAY_BAR_RIGHT_X),
  };
}

// 上下・左右の影勾配を補正
function applyShadowCorrection(canvas: HTMLCanvasElement, bars: GrayBarReadings): void {
  const ctx = canvas.getContext('2d')!;
  const w = canvas.width;
  const h = canvas.height;
  const imgData = ctx.getImageData(0, 0, w, h);
  const data = imgData.data;

  // 各ステップの期待値（0=黒, 0.9=ほぼ白 → 0~229.5）
  const expected: number[] = [];
  for (let i = 0; i < GRAY_BAR_STEPS; i++) {
    expected.push((i / GRAY_BAR_STEPS) * 255);
  }

  // 左右バーの各ステップごとの補正量
  const leftDelta = bars.left.map((v, i) => v - expected[i]);
  const rightDelta = bars.right.map((v, i) => v - expected[i]);

  // バーのピクセル座標
  const scaleY = h / mm(PAGE_HEIGHT);
  const scaleX = w / mm(PAGE_WIDTH);
  const barTopPx = mm(GRAY_BAR_TOP_Y) * scaleY;
  const barBottomPx = mm(GRAY_BAR_BOTTOM_Y) * scaleY;
  const barLeftPx = mm(GRAY_BAR_LEFT_X + GRAY_BAR_STEP_SIZE / 2) * scaleX;
  const barRightPx = mm(GRAY_BAR_RIGHT_X + GRAY_BAR_STEP_SIZE / 2) * scaleX;

  for (let py = 0; py < h; py++) {
    // Y方向: バーのどのステップに近いか（線形補間）
    const barT = Math.max(0, Math.min(1, (py - barTopPx) / (barBottomPx - barTopPx)));
    const stepF = barT * (GRAY_BAR_STEPS - 1);
    const stepLow = Math.floor(stepF);
    const stepHigh = Math.min(stepLow + 1, GRAY_BAR_STEPS - 1);
    const stepFrac = stepF - stepLow;

    // 左バーの補正量（この行のY位置での）
    const leftCorr = leftDelta[stepLow] * (1 - stepFrac) + leftDelta[stepHigh] * stepFrac;
    // 右バーの補正量
    const rightCorr = rightDelta[stepLow] * (1 - stepFrac) + rightDelta[stepHigh] * stepFrac;

    for (let px = 0; px < w; px++) {
      // X方向: 左バー〜右バーの間で線形補間
      const xT = Math.max(0, Math.min(1, (px - barLeftPx) / (barRightPx - barLeftPx)));
      const correction = leftCorr * (1 - xT) + rightCorr * xT;

      const i = (py * w + px) * 4;
      data[i] = Math.max(0, Math.min(255, data[i] - correction));
      data[i + 1] = Math.max(0, Math.min(255, data[i + 1] - correction));
      data[i + 2] = Math.max(0, Math.min(255, data[i + 2] - correction));
    }
  }

  ctx.putImageData(imgData, 0, 0);
}

// マスを切り出して二値化
function extractCell(
  canvas: HTMLCanvasElement,
  row: number,
  col: number,
  cellIndex: number,
): ImageData | null {
  const ctx = canvas.getContext('2d')!;
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);

  const pos = getCellPosition(row, col, cellIndex);
  // 内枠領域のみ切り出す
  const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
  const px = Math.round(mm(pos.x + innerOffset) * scaleX);
  const py = Math.round(mm(pos.y + innerOffset) * scaleY);
  const pw = Math.round(mm(INNER_SIZE) * scaleX);
  const ph = Math.round(mm(INNER_SIZE) * scaleY);

  if (pw <= 0 || ph <= 0) return null;
  return ctx.getImageData(px, py, pw, ph);
}

// 空マス判定（黒ピクセルが1%未満）
function isEmpty(imageData: ImageData): boolean {
  const data = imageData.data;
  const total = data.length / 4;
  let blackCount = 0;
  const threshold = 128;

  for (let i = 0; i < data.length; i += 4) {
    const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
    if (gray < threshold) blackCount++;
  }

  return blackCount / total < 0.01;
}

// チェック欄解析（簡易版: 黒ピクセル密度で✓/×/空欄を推定）
function analyzeCheckMark(
  canvas: HTMLCanvasElement,
  row: number,
  col: number,
  cellIndex: number,
): 'check' | 'cross' | 'empty' {
  const ctx = canvas.getContext('2d')!;
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);

  const pos = getCellPosition(row, col, cellIndex);
  const px = Math.round(mm(pos.x) * scaleX);
  const py = Math.round(mm(pos.y + CELL_SIZE) * scaleY);
  const pw = Math.round(mm(CELL_SIZE) * scaleX);
  const ph = Math.round(mm(CHECK_HEIGHT) * scaleY);

  if (pw <= 0 || ph <= 0) return 'empty';

  const data = ctx.getImageData(px, py, pw, ph).data;
  const total = data.length / 4;
  let blackCount = 0;

  for (let i = 0; i < data.length; i += 4) {
    const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
    if (gray < 128) blackCount++;
  }

  const ratio = blackCount / total;
  if (ratio < 0.02) return 'empty';
  // ✓ vs × の区別は将来的にテンプレートマッチングで改善
  // 暫定: 密度が高ければ×、低ければ✓
  if (ratio > 0.15) return 'cross';
  return 'check';
}

// メイン処理
export async function processImages(
  files: File[],
  callbacks: ProcessCallbacks,
): Promise<ProcessResult> {
  const glyphs: VectorGlyph[] = [];

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
    return { glyphs };
  }

  callbacks.onPageStart(0, imageFiles.length);

  for (let fi = 0; fi < imageFiles.length; fi++) {
    callbacks.onPageStart(fi + 1, imageFiles.length);

    let canvas: HTMLCanvasElement;
    try {
      canvas = await loadImageToCanvas(imageFiles[fi]);
    } catch {
      callbacks.onMessage({
        type: 'error',
        text: `ファイル "${imageFiles[fi].name}" を画像として読み込めませんでした。スキップします。`,
      });
      continue;
    }

    // 四隅マーカー検出 → ページ全体を台形補正 → QR読み取り
    const corners = detectMarkers(canvas);
    let corrected: HTMLCanvasElement;

    if (corners) {
      // 向き検出・回転補正
      const rotation = detectOrientation(corners, canvas);
      const rotated = rotation === 0 ? canvas : rotateCanvas(canvas, rotation);
      // 回転後にマーカーを再検出し、ページ全体の四隅を外挿して台形補正
      const rotatedCorners = rotation === 0 ? corners : detectMarkers(rotated);
      const targetW = Math.round(mm(PAGE_WIDTH) * 4); // 約300dpi相当
      const targetH = Math.round(mm(PAGE_HEIGHT) * 4);
      if (rotatedCorners) {
        const pageCorners = extrapolatePageCorners(rotatedCorners);
        corrected = perspectiveTransform(rotated, pageCorners, targetW, targetH);
      } else {
        corrected = rotated;
      }
    } else {
      callbacks.onMessage({
        type: 'warning',
        text: `画像 ${fi + 1} の四隅マーカーが見つかりません。補正なしで処理を継続します。`,
      });
      corrected = canvas;
    }

    // QRコード読み取り（台形補正後のページ全体画像から）
    const qr = readQRFromCanvas(corrected);
    if (!qr) {
      callbacks.onMessage({
        type: 'error',
        text: `画像 ${fi + 1} のQRコードを読み取れませんでした。画像が不鮮明な可能性があります。`,
      });
      continue;
    }

    // グレースケールバー読み取り+影補正（シアン除去の前）
    const bars = readGrayBars(corrected);
    applyShadowCorrection(corrected, bars);

    // シアン除去
    const [cr, cg, cb] = readCyanSample(corrected);
    removeCyan(corrected, cr, cg, cb);

    // 補正後キャンバスをコールバックで通知
    callbacks.onPageCorrected?.(qr.pg, corrected);

    // 各文字を処理（QRに文字リストがあればそれを使用、なければページ番号から導出）
    const pageChars = qr.chars ?? getCharactersForPage(qr.pg - 1);
    for (let ci = 0; ci < pageChars.length; ci++) {
      const row = Math.floor(ci / COLS);
      const col = ci % COLS;
      const char = pageChars[ci];
      const unicode = char.codePointAt(0)!;

      // 2つのマスを評価
      const cells: {
        imageData: ImageData;
        checkMark: 'check' | 'cross' | 'empty';
        index: number;
      }[] = [];

      for (let cellIdx = 0; cellIdx < 2; cellIdx++) {
        const cellData = extractCell(corrected, row, col, cellIdx);
        if (!cellData || isEmpty(cellData)) continue;

        const check = analyzeCheckMark(corrected, row, col, cellIdx);
        if (check === 'cross') continue;

        cells.push({ imageData: cellData, checkMark: check, index: cellIdx });
      }

      if (cells.length === 0) {
        callbacks.onGlyphStatus?.({
          char,
          unicode,
          pageIndex: qr.pg,
          row,
          col,
          status: 'empty',
        });
        continue;
      }

      // 採用判定
      const checked = cells.filter((c) => c.checkMark === 'check');
      const adopted = checked.length > 0 ? checked : [cells[cells.length - 1]];

      // セル画像のData URLを生成（UI表示用）
      const adoptedCell = adopted[0];
      let cellImageDataUrl: string | undefined;
      try {
        const cellCanvas = document.createElement('canvas');
        cellCanvas.width = adoptedCell.imageData.width;
        cellCanvas.height = adoptedCell.imageData.height;
        cellCanvas.getContext('2d')!.putImageData(adoptedCell.imageData, 0, 0);
        cellImageDataUrl = cellCanvas.toDataURL('image/png');
      } catch {
        /* Node.js環境ではスキップ */
      }

      callbacks.onGlyphStatus?.({
        char,
        unicode,
        pageIndex: qr.pg,
        row,
        col,
        status: 'found',
        cellImageDataUrl,
      });

      for (let ai = 0; ai < adopted.length; ai++) {
        const cell = adopted[ai];
        const paths = vectorizeGlyph(cell.imageData);

        const name =
          ai === 0
            ? `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}`
            : `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}.alt${ai}`;

        glyphs.push({
          name,
          unicode: ai === 0 ? unicode : undefined,
          paths,
          advanceWidth: 1000,
        });
      }
    }
  }

  return { glyphs };
}
