/**
 * 結合テスト: 模擬スキャン画像 → 画像処理 → フォント生成 → 検証
 *
 * processImages はブラウザ File API に依存するため、
 * ここでは個別の処理ステップを順に呼んで結合テストとする。
 */
import { describe, it, expect } from 'vitest';
import '../helpers/canvas-polyfill';
import { createCanvas, loadImage } from 'canvas';
import * as fs from 'node:fs';
import * as path from 'node:path';
import opentype from 'opentype.js';

import { readQRFromImageData } from '../../src/lib/scanner/qr-reader';
import { detectMarkers, perspectiveTransform, detectOrientation, rotateCanvas } from '../../src/lib/scanner/marker-detector';
import { getCharactersForPage, HIRAGANA } from '../../src/data/characters';
import { vectorizeGlyph } from '../../src/lib/vectorizer/contour';
import { buildFont, type VectorGlyph } from '../../src/lib/font/builder';
import {
  mm, PAGE_WIDTH, PAGE_HEIGHT,
  COLS, CELL_SIZE, INNER_SIZE,
  CYAN_SAMPLE_X, CYAN_SAMPLE_Y, CYAN_SAMPLE_SIZE,
  getCellPosition,
} from '../../src/lib/template/layout';

const MOCK_DIR = path.join(import.meta.dirname, '..', 'fixtures', 'mock-scans');

// processor.ts の removeCyan / readCyanSample / extractCell / isEmpty を再現
// (元コードはブラウザ Canvas 前提のため、同一ロジックをここで呼ぶ)

function readCyanSample(canvas: any): [number, number, number] {
  const ctx = canvas.getContext('2d');
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);
  const sx = Math.round(mm(CYAN_SAMPLE_X) * scaleX);
  const sy = Math.round(mm(CYAN_SAMPLE_Y) * scaleY);
  const sw = Math.max(1, Math.round(mm(CYAN_SAMPLE_SIZE) * scaleX));
  const sh = Math.max(1, Math.round(mm(CYAN_SAMPLE_SIZE) * scaleY));
  const data = ctx.getImageData(sx, sy, sw, sh).data;
  let r = 0, g = 0, b = 0;
  const count = data.length / 4;
  for (let i = 0; i < data.length; i += 4) {
    r += data[i]; g += data[i + 1]; b += data[i + 2];
  }
  return [Math.round(r / count), Math.round(g / count), Math.round(b / count)];
}

function removeCyan(canvas: any, cr: number, cg: number, cb: number) {
  const ctx = canvas.getContext('2d');
  const imgData = ctx.getImageData(0, 0, canvas.width, canvas.height);
  const data = imgData.data;
  const tolerance = 80;
  for (let i = 0; i < data.length; i += 4) {
    const dr = data[i] - cr;
    const dg = data[i + 1] - cg;
    const db = data[i + 2] - cb;
    if (Math.sqrt(dr * dr + dg * dg + db * db) < tolerance) {
      data[i] = 255; data[i + 1] = 255; data[i + 2] = 255;
    }
  }
  ctx.putImageData(imgData, 0, 0);
}

function extractCell(canvas: any, row: number, col: number, cellIndex: number) {
  const ctx = canvas.getContext('2d');
  const scaleX = canvas.width / mm(PAGE_WIDTH);
  const scaleY = canvas.height / mm(PAGE_HEIGHT);
  const pos = getCellPosition(row, col, cellIndex);
  const innerOffset = (CELL_SIZE - INNER_SIZE) / 2;
  const px = Math.round(mm(pos.x + innerOffset) * scaleX);
  const py = Math.round(mm(pos.y + innerOffset) * scaleY);
  const pw = Math.round(mm(INNER_SIZE) * scaleX);
  const ph = Math.round(mm(INNER_SIZE) * scaleY);
  if (pw <= 0 || ph <= 0) return null;
  return ctx.getImageData(px, py, pw, ph);
}

function isEmpty(imageData: any): boolean {
  const data = imageData.data;
  const total = data.length / 4;
  let blackCount = 0;
  for (let i = 0; i < data.length; i += 4) {
    const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
    if (gray < 128) blackCount++;
  }
  return blackCount / total < 0.01;
}

describe('Full Pipeline: Mock Scans → Font', () => {
  it('should process 3 hiragana pages and produce a valid TTF', { timeout: 180_000 }, async () => {
    const glyphs: VectorGlyph[] = [];
    const pageFiles = ['page-01.png', 'page-02.png', 'page-03.png'];

    for (const file of pageFiles) {
      const filePath = path.join(MOCK_DIR, file);
      expect(fs.existsSync(filePath)).toBe(true);

      const img = await loadImage(filePath);
      const canvas = createCanvas(img.width, img.height) as any;
      const ctx = canvas.getContext('2d');
      ctx.drawImage(img, 0, 0);

      // 1. QR読み取り
      const headerH = Math.floor(img.height * 0.2);
      const headerData = ctx.getImageData(0, 0, img.width, headerH);
      const qr = readQRFromImageData(headerData as any);
      expect(qr).not.toBeNull();
      expect(qr!.p).toBe('mfc');

      // 2. マーカー検出
      const corners = detectMarkers(canvas);
      expect(corners).not.toBeNull();

      // 3. 向き検出 → 台形補正
      const rotation = detectOrientation(corners!, canvas);
      const rotated = rotation === 0 ? canvas : rotateCanvas(canvas, rotation);
      const rotatedCorners = rotation === 0 ? corners : detectMarkers(rotated);

      const targetW = Math.round(mm(PAGE_WIDTH) * 4);
      const targetH = Math.round(mm(PAGE_HEIGHT) * 4);
      const corrected = rotatedCorners
        ? perspectiveTransform(rotated, rotatedCorners, targetW, targetH)
        : rotated;

      // 4. シアン除去
      const [cr, cg, cb] = readCyanSample(corrected);
      removeCyan(corrected, cr, cg, cb);

      // 5. 各文字を処理
      // テンプレートは「ひらがなのみ」だが、getCharactersForPage は
      // ALL_CHARACTERS（全文字リスト）から取得する。
      // 模擬画像の QR は totalPages=3 でひらがなのみを想定しているので、
      // ページ番号から直接ひらがなリストを使う。
      const start = (qr!.pg - 1) * 30;
      const pageChars = HIRAGANA.slice(start, start + 30);

      for (let ci = 0; ci < pageChars.length; ci++) {
        const row = Math.floor(ci / COLS);
        const col = ci % COLS;
        const char = pageChars[ci];
        const unicode = char.codePointAt(0)!;

        // 1つ目のマスのみ文字が書かれている
        const cellData = extractCell(corrected, row, col, 0);
        if (!cellData || isEmpty(cellData)) continue;

        const paths = vectorizeGlyph(cellData as any);
        if (paths.length === 0) continue;

        const name = `uni${unicode.toString(16).toUpperCase().padStart(4, '0')}`;
        glyphs.push({ name, unicode, paths, advanceWidth: 1000 });
      }
    }

    // 少なくとも一部のひらがなが抽出されたことを確認
    console.log(`Extracted ${glyphs.length} glyphs out of ${HIRAGANA.length} hiragana`);
    expect(glyphs.length).toBeGreaterThan(0);

    // フォント生成
    const fontBuffer = await buildFont({
      familyName: 'TestHiragana',
      glyphs,
    });
    expect(fontBuffer.byteLength).toBeGreaterThan(0);

    // opentype.js で読み返し
    const font = opentype.parse(fontBuffer);
    expect(font.names.fontFamily?.en).toBe('TestHiragana');
    expect(font.unitsPerEm).toBe(1000);

    // .notdef + space + 抽出グリフ数
    expect(font.glyphs.length).toBe(2 + glyphs.length);

    // 代表的なひらがなが含まれているか確認
    const sampleChars = ['あ', 'い', 'う', 'か', 'さ', 'た', 'な', 'は', 'ま'];
    let foundCount = 0;
    for (const ch of sampleChars) {
      const glyph = font.charToGlyph(ch);
      if (glyph && glyph.name !== '.notdef') {
        foundCount++;
      }
    }
    console.log(`Found ${foundCount}/${sampleChars.length} sample hiragana in font`);
    expect(foundCount).toBeGreaterThan(0);

    // フォントファイルとして書き出し（デバッグ用・CI ではアーティファクトとして保存可能）
    const outPath = path.join(import.meta.dirname, '..', 'fixtures', 'test-output.ttf');
    fs.writeFileSync(outPath, Buffer.from(fontBuffer));
    console.log(`Font written to: ${outPath} (${Math.round(fontBuffer.byteLength / 1024)}KB)`);
  });
});
