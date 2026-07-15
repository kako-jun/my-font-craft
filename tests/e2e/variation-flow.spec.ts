/**
 * 実写ループ拡張 4軸バリエーション e2e（#113）
 *
 * 「実写に近いが回復可能」な劣化を乗せた合成テンプレート画像を実ブラウザ
 * パイプラインに通し、golden path が完走する（or 要確認付きで完走する）ことを
 * 検証する。各軸は generate-mock-scans.ts の対応ジェネレータが出力する:
 * - 軸1 手書き風揺らぎ: tests/fixtures/mock-scans-jitter/
 * - 軸2 ノイズ（ごま塩・微小スペック）: tests/fixtures/mock-scans-noise/
 * - 軸3 照明（明度勾配 + 影の帯 + コントラスト低下）: tests/fixtures/mock-scans-lighting/
 * - 軸4 撮影複合（台形 + 回転 + 縮小 + 軽ぼかし）: tests/fixtures/mock-scans-capture/
 *
 * 合格バー: [scan:font-input] ok（＝フォント生成入力まで到達）ログが出て
 * TTF が生成され、そのページの全文字グリフが無傷なこと。段階診断ログ（[scan:*]）が
 * 収集されているので、落ちた場合はどの段階かを局所化できる。
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
const FIXTURES_DIR = path.join(__dirname, '..', 'fixtures');

// ページ1に描画される文字（HIRAGANA の先頭 CHARS_PER_PAGE 文字）
const PAGE1_CHARS = HIRAGANA.slice(0, CHARS_PER_PAGE);

/** 各軸の定義: フィクスチャ dir と suffix */
const AXES = [
  { name: '軸1 手書き風揺らぎ', dir: 'mock-scans-jitter', suffix: '-jitter.png' },
  { name: '軸2 ノイズ（ごま塩・スペック）', dir: 'mock-scans-noise', suffix: '-noise.png' },
  { name: '軸3 照明（明度勾配 + 影の帯）', dir: 'mock-scans-lighting', suffix: '-lighting.png' },
  {
    name: '軸4 撮影複合（台形+回転+縮小+ぼかし）',
    dir: 'mock-scans-capture',
    suffix: '-capture.png',
  },
] as const;

test.describe('実写ループ 4軸バリエーション: 劣化入力でも回復して完走する (#113)', () => {
  for (const axis of AXES) {
    test(`${axis.name}: 全ページから83文字全部を抽出してフォント生成まで完走する`, async ({
      page,
    }, testInfo) => {
      test.setTimeout(300_000);

      const axisDir = path.join(FIXTURES_DIR, axis.dir);
      const files = fs
        .readdirSync(axisDir)
        .filter((f) => f.endsWith(axis.suffix))
        .sort()
        .map((f) => path.join(axisDir, f));
      expect(files.length).toBeGreaterThanOrEqual(2);

      const zipPath = path.join(FIXTURES_DIR, `test-upload-${axis.dir}.zip`);
      await createZipFromFiles(files, zipPath);

      try {
        const { logs } = await withStageLogs(page, testInfo, async () => {
          const fontPath = await runScanToFontFlow(page, zipPath);
          const font = loadFont(fontPath);
          expectGlyphsForChars(font, HIRAGANA);
        });

        // 完走の証跡: [scan:font-input] ok（フォント生成入力まで到達）が各ページ分出ている
        const okLogs = logs.filter((l) => /\[scan:font-input\] ok/.test(l.text));
        expect(okLogs.length, '[scan:font-input] ok ログが出ていない（未完走）').toBeGreaterThan(0);

        // 段階診断ログが収集されていること（落ちたとき局所化できる前提の担保）
        expect(
          logs.some((l) => l.text.startsWith('[scan:')),
          '段階診断ログが無い',
        ).toBe(true);
      } finally {
        if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
      }
    });
  }

  test('撮影複合（capture）の1ページからページ1の全文字を抽出できる', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    const captureDir = path.join(FIXTURES_DIR, 'mock-scans-capture');
    const zipPath = path.join(FIXTURES_DIR, 'test-upload-capture-single.zip');
    await createZipFromFiles([path.join(captureDir, 'page-01-capture.png')], zipPath);

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
