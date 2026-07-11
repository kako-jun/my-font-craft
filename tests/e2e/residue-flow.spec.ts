/**
 * 残渣注入スキャン e2e（#110: セル品質ゲート）
 *
 * 各ページ先頭 RESIDUE_INJECT_CHARS_PER_PAGE 文字の記入マスに、セル境界に
 * 接触する枠残渣風の黒線 + 内側の微小スペックを注入した合成ページをアップロードし、
 * - 全ひらがな83文字のグリフが正しく抽出される（残渣がストロークを壊さない）
 * - 注入セルの文字が review UI に「要確認」フラグ付きで現れる（黙って空に倒さない）
 * - 段階診断ログ [scan:cells] に review= 件数が出る
 * を検証する。残渣の除去自体（グリフに混入しないこと）のピクセル精度の検証は
 * Rust 側 unit テスト（vectorize_strips_border_residue_keeps_stroke_intact）が担う。
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { HIRAGANA, CHARS_PER_PAGE } from '../../src/data/characters';
import { RESIDUE_INJECT_CHARS_PER_PAGE } from '../fixtures/generate-mock-scans';
import {
  createZipFromFiles,
  expectGlyphsForChars,
  loadFont,
  withStageLogs,
} from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const RESIDUE_SCANS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans-residue');

/** 残渣を注入した文字（= 要確認フラグが立つべき文字）をページ構成から計算する */
function expectedReviewChars(): string[] {
  const totalPages = Math.ceil(HIRAGANA.length / CHARS_PER_PAGE);
  const chars: string[] = [];
  for (let pageIdx = 0; pageIdx < totalPages; pageIdx++) {
    const start = pageIdx * CHARS_PER_PAGE;
    const pageChars = HIRAGANA.slice(start, start + CHARS_PER_PAGE);
    chars.push(...pageChars.slice(0, RESIDUE_INJECT_CHARS_PER_PAGE));
  }
  return chars;
}

test.describe('残渣注入スキャン: セル品質ゲート (#110)', () => {
  test('枠残渣が除去され、注入セルが「要確認」として review UI に現れる', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    const files = fs
      .readdirSync(RESIDUE_SCANS_DIR)
      .filter((f) => f.endsWith('.png'))
      .sort()
      .map((f) => path.join(RESIDUE_SCANS_DIR, f));
    expect(files.length).toBeGreaterThanOrEqual(2);

    const zipPath = path.join(RESIDUE_SCANS_DIR, '..', 'test-upload-residue.zip');
    await createZipFromFiles(files, zipPath);

    const reviewChars = expectedReviewChars();

    try {
      await withStageLogs(page, testInfo, async (logs) => {
        await page.goto('/');
        await page.getByRole('link', { name: '2. フォント作成', exact: true }).click();
        await expect(page.locator('h2')).toContainText('フォントを作成する');

        await page.locator('#zip-input').setInputFiles(zipPath);

        // スキャン完了（review フェーズ）を待つ
        await expect(
          page.locator('button', { hasText: /フォントを生成|このまま生成/ }),
        ).toBeVisible({ timeout: 180_000 });

        // 要確認フラグ: 注入セルの数だけ review マークが付き、それ以外には付かない
        await expect(page.locator('.scan-grid__cell--review')).toHaveCount(reviewChars.length);
        for (const char of reviewChars) {
          await expect(
            page
              .locator('.scan-grid__cell--review')
              .filter({ has: page.locator('.scan-grid__cell-char', { hasText: char }) }),
            `文字「${char}」が要確認として表示されるべき`,
          ).toHaveCount(1);
        }

        // 段階診断ログ: [scan:cells] に各ページの review 件数が出ている
        const pageCount = files.length;
        for (let p = 1; p <= pageCount; p++) {
          expect(
            logs.some((l) =>
              new RegExp(
                `\\[scan:cells\\] page=${p} .*review=${RESIDUE_INJECT_CHARS_PER_PAGE}`,
              ).test(l.text),
            ),
            `[scan:cells] page=${p} の review=${RESIDUE_INJECT_CHARS_PER_PAGE} ログがない`,
          ).toBe(true);
        }

        // 要確認セルを検分ビューで「書き直し」に仕分けると要確認表示（明滅クラス + !バッジ）が消える
        // 操作モデル（#114）: セルをタップ → 検分ビュー → X キーで書き直し → Esc で俯瞰へ
        const targetChar = reviewChars[0];
        const targetCell = page
          .locator('.scan-grid__cell')
          .filter({ has: page.locator('.scan-grid__cell-char', { hasText: targetChar }) });
        await expect(targetCell).toHaveCount(1);
        await targetCell.click();
        await expect(page.locator('.inspector')).toBeVisible();
        await page.keyboard.press('x');
        await page.keyboard.press('Escape');
        await expect(page.locator('.inspector')).toHaveCount(0);
        await expect(targetCell).not.toHaveClass(/scan-grid__cell--review/);
        await expect(targetCell.locator('.scan-grid__cell-review-mark')).toHaveCount(0);
        await expect(page.locator('.scan-grid__cell--review')).toHaveCount(reviewChars.length - 1);
        // Enter（採用）で復帰させて、以降のグリフ検証（全83文字）に影響させない
        await targetCell.click();
        await expect(page.locator('.inspector')).toBeVisible();
        await page.keyboard.press('Enter');
        await page.keyboard.press('Escape');
        await expect(page.locator('.inspector')).toHaveCount(0);
        await expect(page.locator('.scan-grid__cell--review')).toHaveCount(reviewChars.length);

        // フォント生成まで進め、全83文字のグリフが無傷なことを確認
        await page.click('button:has-text("フォントを生成"), button:has-text("このまま生成")');
        await expect(page.locator('text=フォントが完成しました')).toBeVisible({
          timeout: 90_000,
        });

        const downloadPromise = page.waitForEvent('download');
        await page.click('text=フォントをダウンロード');
        const download = await downloadPromise;
        const fontPath = await download.path();
        expect(fontPath).toBeTruthy();

        const font = loadFont(fontPath!);
        expectGlyphsForChars(font, HIRAGANA);

        // 成功パス: [scan:*] のエラーログ（採用セルのパスが空 等）が出ていない
        const scanErrors = logs.filter((l) => l.type === 'error' && l.text.startsWith('[scan:'));
        expect(scanErrors, '成功パスなのに [scan:*] エラーログがある').toEqual([]);
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
