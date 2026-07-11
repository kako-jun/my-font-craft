/**
 * 歪み合成スキャン e2e（#109）
 *
 * 斜め撮影風に歪ませた合成テンプレート画像（tests/fixtures/mock-scans-distorted/）を
 * アップロードし、QR/マーカー検出 → 台形補正 → セル切り出し → 二値化 → ベクター化 →
 * フォント生成の golden path が通ることを検証する。
 *
 * バリアント（tests/fixtures/distort.ts の DISTORT_VARIANTS）:
 * - perspective: 台形変形（上辺が狭い）+ グレー背景 + 余白
 * - rotated:     3° 回転 + グレー背景 + 余白
 * - combined:    軽い回転 + 軽い台形 + 縮小 + 明度ムラ + 軽いぼかし
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { HIRAGANA, CHARS_PER_PAGE } from '../../src/data/characters';
import {
  createZipFromFiles,
  expectGlyphsForChars,
  loadFont,
  runScanToFontFlow,
  withStageLogs,
} from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const DISTORTED_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans-distorted');

// ページ1に描画される文字（HIRAGANA の先頭 CHARS_PER_PAGE 文字）
const PAGE1_CHARS = HIRAGANA.slice(0, CHARS_PER_PAGE);

test.describe('歪み合成スキャン: 台形補正を通してフォント生成まで到達する', () => {
  test('台形変形（perspective）の全ページから83文字全部を抽出できる', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    const files = fs
      .readdirSync(DISTORTED_DIR)
      .filter((f) => f.endsWith('-perspective.png'))
      .sort()
      .map((f) => path.join(DISTORTED_DIR, f));
    expect(files.length).toBeGreaterThanOrEqual(2);

    const zipPath = path.join(DISTORTED_DIR, '..', 'test-upload-perspective.zip');
    await createZipFromFiles(files, zipPath);

    try {
      await withStageLogs(page, testInfo, async () => {
        const fontPath = await runScanToFontFlow(page, zipPath);
        const font = loadFont(fontPath);
        expectGlyphsForChars(font, HIRAGANA);
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });

  test('回転（rotated）の1ページからページ1の全文字を抽出できる', async ({ page }, testInfo) => {
    test.setTimeout(300_000);

    const zipPath = path.join(DISTORTED_DIR, '..', 'test-upload-rotated.zip');
    await createZipFromFiles([path.join(DISTORTED_DIR, 'page-01-rotated.png')], zipPath);

    try {
      await withStageLogs(page, testInfo, async () => {
        const fontPath = await runScanToFontFlow(page, zipPath);
        const font = loadFont(fontPath);
        expectGlyphsForChars(font, PAGE1_CHARS);
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });

  test('複合歪み（combined）の1ページからページ1の全文字を抽出できる', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    const zipPath = path.join(DISTORTED_DIR, '..', 'test-upload-combined.zip');
    await createZipFromFiles([path.join(DISTORTED_DIR, 'page-01-combined.png')], zipPath);

    try {
      await withStageLogs(page, testInfo, async () => {
        const fontPath = await runScanToFontFlow(page, zipPath);
        const font = loadFont(fontPath);
        expectGlyphsForChars(font, PAGE1_CHARS);
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
