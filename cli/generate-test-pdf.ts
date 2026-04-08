/**
 * #33 新マーカーレイアウトでテスト用PDFを生成する
 * pdf-lib を使い、layout.ts の定数で描画（ハードコードなし）
 *
 * Usage: npx tsx cli/generate-test-pdf.ts [output.pdf]
 */

import { PDFDocument, rgb } from 'pdf-lib';
import QRCode from 'qrcode';
import fs from 'node:fs';

// --- レイアウト定数（layout.ts から取得） ---
import {
  mm,
  PAGE_WIDTH,
  PAGE_HEIGHT,
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
  CENTER_MARKER,
  getCellPosition,
} from '../src/lib/template/layout';

async function main() {
  const outputPath = process.argv[2] || 'cli/debug_output/test-template.pdf';

  const pdfDoc = await PDFDocument.create();
  const page = pdfDoc.addPage([mm(PAGE_WIDTH), mm(PAGE_HEIGHT)]);

  // PDF座標系は左下原点
  const toY = (mmY: number) => mm(PAGE_HEIGHT) - mm(mmY);

  // --- 四隅マーカー（円） ---
  for (const [, marker] of Object.entries(MARKERS)) {
    const cx = mm(marker.x + MARKER_SIZE / 2);
    const cy = toY(marker.y + MARKER_SIZE / 2);
    const r = mm(MARKER_SIZE / 2);

    if (marker.filled) {
      // 塗りつぶし円: pdf-lib には drawCircle がないので楕円で代用
      page.drawEllipse({
        x: cx,
        y: cy,
        xScale: r,
        yScale: r,
        color: rgb(0, 0, 0),
      });
    } else {
      // 枠線のみの円
      page.drawEllipse({
        x: cx,
        y: cy,
        xScale: r,
        yScale: r,
        borderColor: rgb(0, 0, 0),
        borderWidth: 1.5,
      });
    }
  }

  // --- グレースケールバー ---
  const barHeight = GRAY_BAR_BOTTOM_Y - GRAY_BAR_TOP_Y;
  const stepHeight = barHeight / GRAY_BAR_STEPS;
  for (let i = 0; i < GRAY_BAR_STEPS; i++) {
    const intensity = i / GRAY_BAR_STEPS;
    const y = GRAY_BAR_TOP_Y + i * stepHeight;
    // 左バー
    page.drawRectangle({
      x: mm(GRAY_BAR_LEFT_X),
      y: toY(y + stepHeight),
      width: mm(GRAY_BAR_STEP_SIZE),
      height: mm(stepHeight),
      color: rgb(intensity, intensity, intensity),
    });
    // 右バー
    page.drawRectangle({
      x: mm(GRAY_BAR_RIGHT_X),
      y: toY(y + stepHeight),
      width: mm(GRAY_BAR_STEP_SIZE),
      height: mm(stepHeight),
      color: rgb(intensity, intensity, intensity),
    });
  }

  // --- シアンサンプル ---
  page.drawRectangle({
    x: mm(CYAN_SAMPLE_X),
    y: toY(CYAN_SAMPLE_Y + CYAN_SAMPLE_SIZE),
    width: mm(CYAN_SAMPLE_SIZE),
    height: mm(CYAN_SAMPLE_SIZE),
    color: rgb(0.8, 1, 1),
  });

  // --- QRコード ---
  const qrData = JSON.stringify({ p: 'mfc', v: 2, pg: 1, t: 1, m: 2 });
  try {
    const qrDataUrl = await QRCode.toDataURL(qrData, {
      errorCorrectionLevel: 'H',
      margin: 0,
      width: 256,
    });
    const qrBase64 = qrDataUrl.split(',')[1];
    const qrBytes = Uint8Array.from(atob(qrBase64), (c) => c.charCodeAt(0));
    const qrImage = await pdfDoc.embedPng(qrBytes);
    page.drawImage(qrImage, {
      x: mm(QR_X),
      y: toY(QR_Y + QR_SIZE),
      width: mm(QR_SIZE),
      height: mm(QR_SIZE),
    });
  } catch (e) {
    console.warn('QR generation failed:', e);
  }

  // --- セルグリッド ---
  const cyanColor = rgb(0.8, 1, 1);
  for (let row = 0; row < ROWS; row++) {
    for (let col = 0; col < COLS; col++) {
      for (let cellIdx = 0; cellIdx < 2; cellIdx++) {
        const pos = getCellPosition(row, col, cellIdx);

        // 外枠（黒）
        page.drawRectangle({
          x: mm(pos.x),
          y: toY(pos.y + CELL_SIZE),
          width: mm(CELL_SIZE),
          height: mm(CELL_SIZE),
          borderColor: rgb(0, 0, 0),
          borderWidth: 0.5,
        });

        // 内枠（シアン）
        const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
        page.drawRectangle({
          x: mm(pos.x + innerOffset),
          y: toY(pos.y + innerOffset + INNER_SIZE),
          width: mm(INNER_SIZE),
          height: mm(INNER_SIZE),
          borderColor: cyanColor,
          borderWidth: 0.5,
        });

        // チェック欄
        page.drawRectangle({
          x: mm(pos.x),
          y: toY(pos.y + CELL_SIZE + CHECK_HEIGHT),
          width: mm(CELL_SIZE),
          height: mm(CHECK_HEIGHT),
          borderColor: rgb(0, 0, 0),
          borderWidth: 0.5,
        });
      }
    }
  }

  // --- 中心マーカー（グリッド後に描画して罫線を上書き） ---
  // 白アイソレーション境界（1mm）
  const cmBorder = 1;
  page.drawRectangle({
    x: mm(CENTER_MARKER.x - cmBorder),
    y: toY(CENTER_MARKER.y + CENTER_MARKER.size + cmBorder),
    width: mm(CENTER_MARKER.size + cmBorder * 2),
    height: mm(CENTER_MARKER.size + cmBorder * 2),
    color: rgb(1, 1, 1),
  });
  // 塗りつぶし四角
  page.drawRectangle({
    x: mm(CENTER_MARKER.x),
    y: toY(CENTER_MARKER.y + CENTER_MARKER.size),
    width: mm(CENTER_MARKER.size),
    height: mm(CENTER_MARKER.size),
    color: rgb(0, 0, 0),
  });

  // 保存
  const pdfBytes = await pdfDoc.save();
  const dir = outputPath.substring(0, outputPath.lastIndexOf('/'));
  if (dir) fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(outputPath, pdfBytes);
  console.log(`PDF生成完了: ${outputPath} (${pdfBytes.length} bytes)`);
}

main().catch(console.error);
