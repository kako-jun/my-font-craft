/**
 * 模擬スキャン画像の生成スクリプト
 *
 * テンプレートPDFと同一レイアウトの画像をnode-canvasで生成し、
 * 各マスの内枠にフォントで文字を描画する。
 * QRコード・四隅マーカー・シアン要素も含める。
 *
 * 出力: tests/fixtures/mock-scans/ に PNG ファイル
 */

import { createCanvas } from 'canvas';
import * as fs from 'node:fs';
import * as path from 'node:path';
import QRCode from 'qrcode';

// --- レイアウト定数（layout.ts と同一値） ---
const MM_TO_PT = 72 / 25.4;
const mm = (v: number) => v * MM_TO_PT;

const PAGE_WIDTH = 210;
const PAGE_HEIGHT = 297;
const MARGIN = 10;
const HEADER_HEIGHT = 15;
const BODY_START_X = MARGIN;
const BODY_START_Y = MARGIN + HEADER_HEIGHT + 5;
const COLS = 4;
const ROWS = 12;
const COL_WIDTH = 47;
const ROW_HEIGHT = 20;
const CELL_SIZE = 15;
const INNER_SIZE = 10;
const CHECK_HEIGHT = 3;
const CELL_GAP = 2;
const SAMPLE_WIDTH = 10;
const MARKER_SIZE = 5;
const QR_X = 10;
const QR_Y = 10;
const QR_SIZE = 12;
const GRAY_BAR_X = 120;
const GRAY_BAR_Y = 10;
const GRAY_BAR_STEP_W = 5;
const GRAY_BAR_STEP_H = 5;
const GRAY_BAR_STEPS = 10;
const CYAN_SAMPLE_X = 175;
const CYAN_SAMPLE_Y = 10;
const CYAN_SAMPLE_SIZE = 5;

const MARKERS = {
  topLeft: { x: 10, y: 25, filled: true },
  topRight: { x: 195, y: 25, filled: false },
  bottomLeft: { x: 10, y: 287, filled: false },
  bottomRight: { x: 195, y: 287, filled: false },
} as const;

// ひらがな（characters.ts と同じ順序）
const HIRAGANA = [
  'あ','い','う','え','お',
  'か','き','く','け','こ',
  'さ','し','す','せ','そ',
  'た','ち','つ','て','と',
  'な','に','ぬ','ね','の',
  'は','ひ','ふ','へ','ほ',
  'ま','み','む','め','も',
  'や','ゆ','よ',
  'ら','り','る','れ','ろ',
  'わ','を','ん',
  'が','ぎ','ぐ','げ','ご',
  'ざ','じ','ず','ぜ','ぞ',
  'だ','ぢ','づ','で','ど',
  'ば','び','ぶ','べ','ぼ',
  'ぱ','ぴ','ぷ','ぺ','ぽ',
  'ぁ','ぃ','ぅ','ぇ','ぉ',
  'っ',
  'ゃ','ゅ','ょ',
  'ゎ',
  'ゐ','ゑ',
];

const CHARS_PER_PAGE = 48;

function getCellPosition(row: number, col: number, cellIndex: number) {
  const x = BODY_START_X + col * COL_WIDTH + SAMPLE_WIDTH + CELL_GAP + cellIndex * (CELL_SIZE + CELL_GAP);
  const y = BODY_START_Y + row * ROW_HEIGHT;
  return { x, y };
}

// 解像度: 300dpi 相当（mm→pixel 変換）
const DPI = 300;
const MM_TO_PX = DPI / 25.4;
const px = (mmVal: number) => Math.round(mmVal * MM_TO_PX);

const canvasW = px(PAGE_WIDTH);
const canvasH = px(PAGE_HEIGHT);

/**
 * 星形マーカーを描画（十字型で近似、generator.ts と同じロジック）
 */
function drawStarMarker(
  ctx: CanvasRenderingContext2D,
  xMm: number, yMm: number, sizeMm: number, filled: boolean,
) {
  const unit = sizeMm / 5;
  ctx.fillStyle = '#000000';
  ctx.strokeStyle = '#000000';
  ctx.lineWidth = 1;

  if (filled) {
    // 縦棒
    ctx.fillRect(px(xMm + unit), px(yMm), px(unit * 3), px(sizeMm));
    // 横棒
    ctx.fillRect(px(xMm), px(yMm + unit), px(sizeMm), px(unit * 3));
  } else {
    // 縦棒（枠線のみ）
    ctx.strokeRect(px(xMm + unit), px(yMm), px(unit * 3), px(sizeMm));
    // 横棒（枠線のみ）
    ctx.strokeRect(px(xMm), px(yMm + unit), px(sizeMm), px(unit * 3));
  }
}

async function generatePage(pageIdx: number, chars: string[]): Promise<Buffer> {
  const canvas = createCanvas(canvasW, canvasH);
  const ctx = canvas.getContext('2d');

  // 白背景
  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, canvasW, canvasH);

  // --- ヘッダー ---
  // タイトル
  ctx.fillStyle = '#000000';
  ctx.font = '28px sans-serif';
  ctx.fillText('MyFontCraft', px(25), px(14));

  // ページ番号
  const totalPages = Math.ceil(HIRAGANA.length / CHARS_PER_PAGE);
  ctx.font = '24px sans-serif';
  ctx.fillText(`Page ${pageIdx + 1}/${totalPages}`, px(80), px(14));

  // QRコード
  const qrData = JSON.stringify({
    p: 'mfc',
    v: 1,
    pg: pageIdx + 1,
    t: totalPages,
    m: 2,
  });
  try {
    const qrBuffer = await QRCode.toBuffer(qrData, {
      errorCorrectionLevel: 'M',
      margin: 0,
      width: px(QR_SIZE),
      type: 'png',
    });
    const { createCanvas: _, loadImage } = await import('canvas');
    const qrImg = await loadImage(qrBuffer);
    ctx.drawImage(qrImg, px(QR_X), px(QR_Y), px(QR_SIZE), px(QR_SIZE));
  } catch (e) {
    console.warn(`QR generation failed for page ${pageIdx + 1}:`, e);
  }

  // グレースケールバー
  for (let i = 0; i < GRAY_BAR_STEPS; i++) {
    const intensity = Math.round((i / GRAY_BAR_STEPS) * 255);
    ctx.fillStyle = `rgb(${intensity},${intensity},${intensity})`;
    ctx.fillRect(
      px(GRAY_BAR_X + i * GRAY_BAR_STEP_W),
      px(GRAY_BAR_Y),
      px(GRAY_BAR_STEP_W),
      px(GRAY_BAR_STEP_H),
    );
  }

  // シアンサンプル
  ctx.fillStyle = '#99FFFF';
  ctx.fillRect(px(CYAN_SAMPLE_X), px(CYAN_SAMPLE_Y), px(CYAN_SAMPLE_SIZE), px(CYAN_SAMPLE_SIZE));

  // --- 四隅マーカー ---
  for (const marker of Object.values(MARKERS)) {
    drawStarMarker(ctx, marker.x, marker.y, MARKER_SIZE, marker.filled);
  }

  // --- 文字マス ---
  for (let i = 0; i < chars.length; i++) {
    const row = Math.floor(i / COLS);
    const col = i % COLS;
    const char = chars[i];

    // 2つのマス
    for (let cellIdx = 0; cellIdx < 2; cellIdx++) {
      const pos = getCellPosition(row, col, cellIdx);

      // 外枠（黒）
      ctx.strokeStyle = '#000000';
      ctx.lineWidth = 2;
      ctx.strokeRect(px(pos.x), px(pos.y), px(CELL_SIZE), px(CELL_SIZE));

      // 内枠（シアン）
      const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
      ctx.strokeStyle = '#99FFFF';
      ctx.lineWidth = 1;
      ctx.strokeRect(
        px(pos.x + innerOffset),
        px(pos.y + innerOffset),
        px(INNER_SIZE),
        px(INNER_SIZE),
      );

      // チェック欄区切り（シアン）
      ctx.beginPath();
      ctx.moveTo(px(pos.x), px(pos.y + CELL_SIZE));
      ctx.lineTo(px(pos.x + CELL_SIZE), px(pos.y + CELL_SIZE));
      ctx.strokeStyle = '#99FFFF';
      ctx.lineWidth = 1;
      ctx.stroke();

      // チェック欄外枠
      ctx.strokeStyle = '#000000';
      ctx.lineWidth = 1;
      ctx.strokeRect(px(pos.x), px(pos.y + CELL_SIZE), px(CELL_SIZE), px(CHECK_HEIGHT));

      // --- 文字を描画（1つ目のマスにのみ書く。2つ目は空欄） ---
      if (cellIdx === 0) {
        const fontSize = px(INNER_SIZE * 0.75);
        ctx.fillStyle = '#000000';
        ctx.font = `${fontSize}px "Hiragino Sans", "Noto Sans CJK JP", sans-serif`;
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        const cx = px(pos.x + innerOffset) + px(INNER_SIZE) / 2;
        const cy = px(pos.y + innerOffset) + px(INNER_SIZE) / 2;
        ctx.fillText(char, cx, cy);
        ctx.textAlign = 'start';
        ctx.textBaseline = 'alphabetic';

        // チェック欄に✓を描画（黒で。シアン除去後も残る）
        const checkCx = px(pos.x + 3);
        const checkCy = px(pos.y + CELL_SIZE + CHECK_HEIGHT / 2);
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(checkCx, checkCy);
        ctx.lineTo(checkCx + px(2), checkCy + px(1.2));
        ctx.lineTo(checkCx + px(5), checkCy - px(1));
        ctx.stroke();
      }
    }
  }

  return canvas.toBuffer('image/png');
}

export async function generateMockScans(outputDir: string): Promise<string[]> {
  fs.mkdirSync(outputDir, { recursive: true });

  const totalPages = Math.ceil(HIRAGANA.length / CHARS_PER_PAGE);
  const files: string[] = [];

  for (let pageIdx = 0; pageIdx < totalPages; pageIdx++) {
    const start = pageIdx * CHARS_PER_PAGE;
    const pageChars = HIRAGANA.slice(start, start + CHARS_PER_PAGE);

    const buffer = await generatePage(pageIdx, pageChars);
    const filename = `page-${String(pageIdx + 1).padStart(2, '0')}.png`;
    const filepath = path.join(outputDir, filename);
    fs.writeFileSync(filepath, buffer);
    files.push(filepath);
    console.log(`Generated: ${filename} (${pageChars.length} chars: ${pageChars[0]}〜${pageChars[pageChars.length - 1]})`);
  }

  return files;
}

// CLI から直接実行された場合
if (process.argv[1]?.endsWith('generate-mock-scans.ts') || process.argv[1]?.endsWith('generate-mock-scans.js')) {
  const outDir = path.join(import.meta.dirname ?? path.dirname(process.argv[1]), 'mock-scans');
  generateMockScans(outDir).then(files => {
    console.log(`\nDone! Generated ${files.length} mock scan images.`);
  });
}
