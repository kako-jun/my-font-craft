/**
 * 穴あき文字ラスタライズ視覚検証 e2e（#112: 輪郭追跡 + 巻き方向管理）
 *
 * 模擬スキャン（正面）をアップロードしてフォントを生成し、穴あき文字
 * （あ・お・ぬ・ふ・ぼ）のグリフが「外輪郭 + 穴」の複数輪郭を保ったまま
 * 書き出されることを検証する。evenodd/自己交差崩壊（#82/#84）で穴が潰れると
 * 単一輪郭に退化するため、輪郭数（閉サブパス = Z コマンド数）で穴の保存を測る。
 *
 * ピクセル単位の巻き方向検証は Rust 側 unit（contour_annulus_hole_stays_open 等）が担い、
 * ここは「実フォント書き出しまで通しても穴が残る」ことの e2e 保証を与える。
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { runScanToFontFlow, loadFont } from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const MOCK_SCANS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans');

/** 明確な穴を持つひらがな（内側ループが1つ以上ある字形） */
const HOLED_CHARS = ['あ', 'お', 'ぬ', 'ふ', 'ぼ'];

/** グリフの閉サブパス数（opentype パスコマンドの 'Z' の数）を数える */
function closedContourCount(font: ReturnType<typeof loadFont>, char: string): number {
  const glyph = font.charToGlyph(char);
  expect(glyph, `グリフ「${char}」が存在する`).toBeTruthy();
  expect(glyph.index, `グリフ「${char}」が .notdef でない`).toBeGreaterThan(0);
  const cmds = glyph.path?.commands ?? [];
  return cmds.filter((c) => c.type === 'Z').length;
}

test.describe('穴あき文字ラスタライズ視覚検証 (#112)', () => {
  test('穴あき文字が「外輪郭+穴」の複数輪郭を保つ', async ({ page }) => {
    test.setTimeout(300_000);

    const files = fs
      .readdirSync(MOCK_SCANS_DIR)
      .filter((f) => f.endsWith('.png'))
      .sort()
      .map((f) => path.join(MOCK_SCANS_DIR, f));
    expect(files.length).toBeGreaterThanOrEqual(2);

    const zipPath = path.join(MOCK_SCANS_DIR, '..', 'test-upload-holed.zip');
    const { createZipFromFiles } = await import('./font-flow-utils');
    await createZipFromFiles(files, zipPath);

    try {
      const fontPath = await runScanToFontFlow(page, zipPath);
      const font = loadFont(fontPath);

      // 穴あき文字は少なくとも1字が複数輪郭（穴が別サブパスとして保存）である。
      // 全字を必須にすると二値化のふらつきで稀に穴が塞がる字が出るため、
      // 「穴あき文字群のうち大半で穴が残る」ことを保証する（視覚崩壊の回帰検知）。
      const counts = HOLED_CHARS.map((c) => ({ c, n: closedContourCount(font, c) }));
      const holed = counts.filter((x) => x.n >= 2);
      expect(
        holed.length,
        `穴あき文字の輪郭数: ${counts.map((x) => `${x.c}=${x.n}`).join(', ')}`,
      ).toBeGreaterThanOrEqual(3);
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
