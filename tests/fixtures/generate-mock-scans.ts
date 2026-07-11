/**
 * 模擬スキャン画像の生成スクリプト
 *
 * テンプレートPDFと同一レイアウトの画像をnode-canvasで生成し、
 * 各マスの内枠にフォントで文字を描画する。
 * QRコード・四隅マーカー・シアン要素も含める。
 *
 * 出力:
 * - tests/fixtures/mock-scans/ に正面 PNG（page-NN.png）
 * - tests/fixtures/mock-scans-distorted/ に斜め撮影風バリアント
 *   （page-NN-perspective.png / page-NN-rotated.png / page-NN-combined.png、#109）
 */

import { createCanvas, registerFont } from 'canvas';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import QRCode from 'qrcode';

// --- フィクスチャ描画フォントの固定（#111 QA） ---
// 環境フォント（macOS の Hiragino 等）に依存すると、glyph-metrics e2e の
// 絶対値アサーション（bbox 位置・大きさ）が実行環境でフレークする。
// OFL 1.1 の Noto Sans CJK JP Regular を かな+ASCII にサブセットして同梱し、
// 描画フォントを決定的にする（fonts/OFL.txt がライセンス）。
// 出自: notofonts/noto-cjk Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf を
//   pyftsubset --unicodes="U+0020-007E,U+3000-303F,U+3040-309F,U+30A0-30FF"
// でサブセット（96KB）。ファイルが無い場合は registerFont が throw するため、
// 環境フォントへのサイレントフォールバックは起きない。
const FIXTURES_DIR = path.dirname(fileURLToPath(import.meta.url));
const FIXTURE_FONT_FAMILY = 'MockScanNotoSansJP';
registerFont(path.join(FIXTURES_DIR, 'fonts', 'NotoSansJP-MockScan.otf'), {
  family: FIXTURE_FONT_FAMILY,
});

// --- レイアウト定数（layout.ts から取得） ---
import {
  PAGE_WIDTH,
  PAGE_HEIGHT,
  MARGIN,
  BODY_START_X,
  BODY_START_Y,
  COLS,
  ROWS,
  COL_WIDTH,
  ROW_HEIGHT,
  CELL_SIZE,
  INNER_SIZE,
  CHECK_HEIGHT,
  CELL_GAP,
  SAMPLE_WIDTH,
  GUIDE_BASELINE_OFFSET,
  MARKER_SIZE,
  MARKERS,
  QR_X,
  QR_Y,
  QR_SIZE,
  GRAY_BAR_STEPS,
  GRAY_BAR_STEP_SIZE,
  GRAY_BAR_LEFT_X,
  GRAY_BAR_RIGHT_X,
  GRAY_BAR_TOP_Y,
  GRAY_BAR_BOTTOM_Y,
  CYAN_SAMPLE_X,
  CYAN_SAMPLE_Y,
  CYAN_SAMPLE_SIZE,
  isSkippedCell,
  gridToCharIndex,
} from '../../src/lib/template/layout';

// ひらがな83文字はテンプレート生成と同じ正本（characters.ts）から取得する（二重管理禁止）
import { HIRAGANA, CHARS_PER_PAGE } from '../../src/data/characters';
import { getCellPosition } from '../../src/lib/template/layout';
import { distortPng, DISTORT_VARIANTS } from './distort';

// 解像度: 300dpi 相当（mm→pixel 変換）
const DPI = 300;
const MM_TO_PX = DPI / 25.4;
const px = (mmVal: number) => Math.round(mmVal * MM_TO_PX);

const canvasW = px(PAGE_WIDTH);
const canvasH = px(PAGE_HEIGHT);

/**
 * 円形マーカーを描画（generator.ts と同じロジック）
 */
function drawCircleMarker(
  ctx: CanvasRenderingContext2D,
  xMm: number,
  yMm: number,
  sizeMm: number,
  filled: boolean,
) {
  const centerX = px(xMm + sizeMm / 2);
  const centerY = px(yMm + sizeMm / 2);
  const radius = px(sizeMm / 2);

  ctx.fillStyle = '#000000';
  ctx.strokeStyle = '#000000';
  ctx.lineWidth = 2;

  ctx.beginPath();
  ctx.arc(centerX, centerY, radius, 0, Math.PI * 2);
  if (filled) {
    ctx.fill();
  } else {
    ctx.stroke();
  }
}

/**
 * 残渣注入バリアント（#110）で各ページの先頭から何文字に残渣を注入するか。
 * e2e（residue-flow.spec.ts）はこの値から要確認フラグの期待文字を計算する。
 */
export const RESIDUE_INJECT_CHARS_PER_PAGE = 5;

/**
 * セルに枠残渣風のノイズを描き込む（#110 品質ゲートの検証用）。
 *
 * Rust 側のセル切り出しは cell 左上から 1.5mm マージンで crop するので、
 * crop 境界（1.5mm）を跨ぐ位置に細い黒線を描くと「セル境界に接触する残渣」になる。
 * - 横線: crop 上端に接触する罫線残渣風（幅8mm、厚み5px）
 * - 縦線: crop 左端に接触するシアン枠残渣風（長さ7mm、厚み5px）
 * - スペック: 内側の 3x3px 黒点（面積フィルタで除去される想定）
 * 黒(#000)で描くのは、シアン除去・罫線残骸除去（輝度150未満は保護）を
 * 意図的にすり抜けさせて「二値化まで生き残る残渣」を再現するため。
 */
function drawCellResidue(ctx: CanvasRenderingContext2D, posX: number, posY: number) {
  ctx.fillStyle = '#000000';
  // 横線: y = 1.35mm（crop 上端 1.5mm を跨ぐ）、x = 3mm から 8mm 分
  ctx.fillRect(px(posX + 3), px(posY + 1.35), px(8), 5);
  // 縦線: x = 1.35mm（crop 左端 1.5mm を跨ぐ）、y = 4mm から 7mm 分
  ctx.fillRect(px(posX + 1.35), px(posY + 4), 5, px(7));
  // 微小スペック: crop 内側（境界非接触）の 3x3px
  ctx.fillRect(px(posX + 3.0), px(posY + 3.0), 3, 3);
}

async function generatePage(
  pageIdx: number,
  chars: string[],
  residueCharIndices?: Set<number>,
  qrDataOverride?: string,
): Promise<Buffer> {
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

  // QRコード（#91: v:3 + s フラグ必須。mock はひらがなのみ。
  // metrics ページは chars ペイロード（#96 リトライPDF形式）で上書きする）
  const qrData =
    qrDataOverride ??
    JSON.stringify({
      p: 'mfc',
      v: 3,
      pg: pageIdx + 1,
      t: totalPages,
      m: 2,
      s: 'h',
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
    ctx.drawImage(qrImg as any, px(QR_X), px(QR_Y), px(QR_SIZE), px(QR_SIZE));
  } catch (e) {
    console.warn(`QR generation failed for page ${pageIdx + 1}:`, e);
  }

  // 左右縦グレースケールバー
  const barHeight = GRAY_BAR_BOTTOM_Y - GRAY_BAR_TOP_Y;
  const stepHeight = barHeight / GRAY_BAR_STEPS;
  for (let i = 0; i < GRAY_BAR_STEPS; i++) {
    const intensity = Math.round((i / GRAY_BAR_STEPS) * 255);
    ctx.fillStyle = `rgb(${intensity},${intensity},${intensity})`;
    const y = GRAY_BAR_TOP_Y + i * stepHeight;
    // 左バー
    ctx.fillRect(px(GRAY_BAR_LEFT_X), px(y), px(GRAY_BAR_STEP_SIZE), px(stepHeight));
    // 右バー
    ctx.fillRect(px(GRAY_BAR_RIGHT_X), px(y), px(GRAY_BAR_STEP_SIZE), px(stepHeight));
  }

  // シアンサンプル
  ctx.fillStyle = '#CCFFFF';
  ctx.fillRect(px(CYAN_SAMPLE_X), px(CYAN_SAMPLE_Y), px(CYAN_SAMPLE_SIZE), px(CYAN_SAMPLE_SIZE));

  // --- 四隅マーカー ---
  for (const marker of Object.values(MARKERS)) {
    drawCircleMarker(ctx, marker.x, marker.y, MARKER_SIZE, marker.filled);
  }

  // --- 文字マス ---
  for (let row = 0; row < ROWS; row++) {
    for (let col = 0; col < COLS; col++) {
      // 中心マーカーが占有するセルはスキップ
      if (isSkippedCell(row, col)) {
        continue;
      }

      const charIdx = gridToCharIndex(row, col);
      if (charIdx === null || charIdx >= chars.length) {
        continue;
      }
      const char = chars[charIdx];

      // 2つのマス
      for (let cellIdx = 0; cellIdx < 2; cellIdx++) {
        const pos = getCellPosition(row, col, cellIdx);

        // 外枠（黒）
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = 2;
        ctx.strokeRect(px(pos.x), px(pos.y), px(CELL_SIZE), px(CELL_SIZE));

        // 内枠（シアン）
        const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
        ctx.strokeStyle = '#CCFFFF';
        ctx.lineWidth = 1;
        ctx.strokeRect(
          px(pos.x + innerOffset),
          px(pos.y + innerOffset),
          px(INNER_SIZE),
          px(INNER_SIZE),
        );

        // ベースライン/センターガイド（#111、シアン=スキャン時に除去される）
        // テンプレートPDF（generator.ts）と同じ位置に描き、除去経路を e2e で検証する
        const baselineY = px(pos.y + innerOffset + INNER_SIZE - GUIDE_BASELINE_OFFSET);
        ctx.strokeStyle = '#CCFFFF';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(px(pos.x + innerOffset), baselineY);
        ctx.lineTo(px(pos.x + innerOffset + INNER_SIZE), baselineY);
        ctx.stroke();
        const centerGuideX = px(pos.x + CELL_SIZE / 2);
        ctx.beginPath();
        ctx.moveTo(centerGuideX, px(pos.y + innerOffset));
        ctx.lineTo(centerGuideX, px(pos.y + innerOffset + INNER_SIZE));
        ctx.stroke();

        // チェック欄区切り（シアン）
        ctx.beginPath();
        ctx.moveTo(px(pos.x), px(pos.y + CELL_SIZE));
        ctx.lineTo(px(pos.x + CELL_SIZE), px(pos.y + CELL_SIZE));
        ctx.strokeStyle = '#CCFFFF';
        ctx.lineWidth = 1;
        ctx.stroke();

        // チェック欄外枠
        ctx.strokeStyle = '#000000';
        ctx.lineWidth = 1;
        ctx.strokeRect(px(pos.x), px(pos.y + CELL_SIZE), px(CELL_SIZE), px(CHECK_HEIGHT));

        // --- 文字を描画（1つ目のマスにのみ書く。2つ目は空欄） ---
        if (cellIdx === 0) {
          // 句読点はフォントグリフのインクが極端に小さく、0.75em 描画だと
          // 空マス判定（内側60%で黒2%未満）と 2.1% vs 2.0% の際どい勝負になる。
          // 手書きの句読点はフォントより相対的に大きく書かれるのが実態なので、
          // 1.0em で描いて閾値境界から離す（#111 metrics ページで使用）
          const scale = '、。'.includes(char) ? 1.0 : 0.75;
          const fontSize = px(INNER_SIZE * scale);
          ctx.fillStyle = '#000000';
          // 同梱サブセットフォントのみ指定（フォールバック列挙しない = 環境差を排除）
          ctx.font = `${fontSize}px "${FIXTURE_FONT_FAMILY}"`;
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

          // 残渣注入バリアント（#110）: 指定文字の記入マスに枠残渣風ノイズを描く
          if (residueCharIndices?.has(charIdx)) {
            drawCellResidue(ctx, pos.x, pos.y);
          }
        }
      }
    }
  }

  return canvas.toBuffer('image/png');
}

/**
 * 出力ディレクトリを用意し、既存の PNG を削除する。
 * ページ数や歪みバリアントが減った時に前回生成の stale なフィクスチャが
 * 残留して e2e に混入するのを防ぐ（ZIP 化は「ディレクトリ内の全 PNG」で拾うため）。
 */
function prepareOutputDir(outputDir: string): void {
  fs.mkdirSync(outputDir, { recursive: true });
  for (const entry of fs.readdirSync(outputDir)) {
    if (entry.endsWith('.png')) {
      fs.unlinkSync(path.join(outputDir, entry));
    }
  }
}

export async function generateMockScans(outputDir: string): Promise<string[]> {
  prepareOutputDir(outputDir);

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
    console.log(
      `Generated: ${filename} (${pageChars.length} chars: ${pageChars[0]}〜${pageChars[pageChars.length - 1]})`,
    );
  }

  return files;
}

/**
 * 残渣注入バリアントを生成する（#110）。
 * 各ページの先頭 RESIDUE_INJECT_CHARS_PER_PAGE 文字の記入マスに、
 * 枠残渣風の細い黒線・微小スペックを描き込んだ正面画像を出力する。
 * 出力先を分けているのは、正面 e2e（mock-scans/ 全 PNG を ZIP 化）に
 * 残渣画像が混入しないようにするため。
 */
export async function generateResidueScans(outputDir: string): Promise<string[]> {
  prepareOutputDir(outputDir);

  const totalPages = Math.ceil(HIRAGANA.length / CHARS_PER_PAGE);
  const residueIndices = new Set(
    Array.from({ length: RESIDUE_INJECT_CHARS_PER_PAGE }, (_, i) => i),
  );
  const files: string[] = [];

  for (let pageIdx = 0; pageIdx < totalPages; pageIdx++) {
    const start = pageIdx * CHARS_PER_PAGE;
    const pageChars = HIRAGANA.slice(start, start + CHARS_PER_PAGE);

    const buffer = await generatePage(pageIdx, pageChars, residueIndices);
    const filename = `page-${String(pageIdx + 1).padStart(2, '0')}-residue.png`;
    const filepath = path.join(outputDir, filename);
    fs.writeFileSync(filepath, buffer);
    files.push(filepath);
    console.log(
      `Generated: ${filename} (residue on ${Math.min(
        RESIDUE_INJECT_CHARS_PER_PAGE,
        pageChars.length,
      )} cells)`,
    );
  }

  return files;
}

/**
 * 配置検証（metrics）ページの文字リスト（#111）。
 * セル→em 固定変換の e2e 検証用: 句読点（、。）・小書きかな（っ vs つ、ぁ vs あ）・
 * descender（g/j/p/q/y）・長音（ー）をフォント描画でセルに載せる。
 * QR は #96 リトライPDF形式の `chars` ペイロードで、この配列をそのまま埋め込む。
 * glyph-metrics-flow.spec.ts と共有する（二重管理しない）。
 */
export const METRICS_PAGE_CHARS = [
  '、',
  '。',
  'っ',
  'つ',
  'ぁ',
  'あ',
  'g',
  'j',
  'p',
  'q',
  'y',
  'ー',
];

/**
 * 配置検証ページを生成する（#111）。
 * 出力先を分けているのは、正面 e2e（mock-scans/ 全 PNG を ZIP 化）の
 * ひらがな83文字検証に metrics ページの重複文字（っ つ ぁ あ）を混入させないため。
 */
export async function generateMetricsScans(outputDir: string): Promise<string[]> {
  prepareOutputDir(outputDir);

  const qrData = JSON.stringify({
    p: 'mfc',
    v: 3,
    pg: 1,
    t: 1,
    m: 2,
    chars: METRICS_PAGE_CHARS,
  });
  const buffer = await generatePage(0, METRICS_PAGE_CHARS, undefined, qrData);
  const filepath = path.join(outputDir, 'page-01-metrics.png');
  fs.writeFileSync(filepath, buffer);
  console.log(`Generated: page-01-metrics.png (${METRICS_PAGE_CHARS.join('')})`);
  return [filepath];
}

/**
 * 正面画像から斜め撮影風の歪みバリアントを生成する（#109）。
 * 出力先を分けているのは、既存の正面 e2e（mock-scans/ 全 PNG を ZIP 化）に
 * 歪み画像が混入しないようにするため。
 */
export async function generateDistortedScans(
  frontalFiles: string[],
  outputDir: string,
): Promise<string[]> {
  prepareOutputDir(outputDir);
  const files: string[] = [];

  for (const frontalPath of frontalFiles) {
    const input = fs.readFileSync(frontalPath);
    const base = path.basename(frontalPath, '.png'); // page-NN
    for (const { suffix, opts } of DISTORT_VARIANTS) {
      const buffer = await distortPng(input, opts);
      const filename = `${base}-${suffix}.png`;
      const filepath = path.join(outputDir, filename);
      fs.writeFileSync(filepath, buffer);
      files.push(filepath);
      console.log(`Generated: ${filename}`);
    }
  }

  return files;
}

// CLI から直接実行された場合
if (
  process.argv[1]?.endsWith('generate-mock-scans.ts') ||
  process.argv[1]?.endsWith('generate-mock-scans.js')
) {
  const fixturesDir = import.meta.dirname ?? path.dirname(process.argv[1]);
  const outDir = path.join(fixturesDir, 'mock-scans');
  const distortedDir = path.join(fixturesDir, 'mock-scans-distorted');
  const residueDir = path.join(fixturesDir, 'mock-scans-residue');
  const metricsDir = path.join(fixturesDir, 'mock-scans-metrics');
  generateMockScans(outDir)
    .then((files) => generateDistortedScans(files, distortedDir).then((d) => [...files, ...d]))
    .then((files) => generateResidueScans(residueDir).then((r) => [...files, ...r]))
    .then((files) => generateMetricsScans(metricsDir).then((m) => [...files, ...m]))
    .then((files) => {
      console.log(`\nDone! Generated ${files.length} mock scan images.`);
    })
    .catch((e) => {
      // 失敗を握りつぶすと test:e2e が古いフィクスチャのまま走ってしまうため、
      // 原因を出力して非0で終了する
      console.error('mock scan generation failed:', e);
      process.exitCode = 1;
    });
}
