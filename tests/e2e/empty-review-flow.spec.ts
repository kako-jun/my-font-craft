/**
 * 「グリフを生成できない採用セル」の要確認可視化 e2e（#112 / #108）
 *
 * 各ページ先頭 EMPTY_REVIEW_INJECT_CHARS_PER_PAGE 文字の記入マスに、採用されるが
 * 品質ゲートで全除去される細線だけを描いた合成ページをアップロードする。
 * この結果セルは「採用（非空）だがベクター化結果が空」になる。#108 の批判
 * （黙って欠字）を防ぐため、こうしたセルは黙って空グリフにせず review UI に
 * 「要確認」として現れなければならない。それを end-to-end で検証する。
 *
 * 注: MAX_CONTOURS / MAX_CONTOUR_POINTS ガードそのものの発火は、セル解像度
 * （約142px 角）では原理的に到達しない（本数・点数の上限がセル画素数を超える）。
 * ハングガード発火時に needs_review が立つ配線（vectorize_adopted_with_review）は
 * Rust 側 unit（contour_explosion_returns_empty / max_contour_points_fires_and_flags_review）
 * が担保する。本 e2e は「採用セルがグリフ化できないとき要確認になる」という同じ
 * ユーザー向け保証をブラウザ経路で固定する。
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { HIRAGANA, CHARS_PER_PAGE } from '../../src/data/characters';
import { EMPTY_REVIEW_INJECT_CHARS_PER_PAGE } from '../fixtures/generate-mock-scans';
import { createZipFromFiles, withStageLogs } from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SCANS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans-emptyreview');

/** 細線注入セル（= 要確認になるべき文字）をページ構成から計算する */
function expectedReviewChars(): string[] {
  const totalPages = Math.ceil(HIRAGANA.length / CHARS_PER_PAGE);
  const chars: string[] = [];
  for (let pageIdx = 0; pageIdx < totalPages; pageIdx++) {
    const start = pageIdx * CHARS_PER_PAGE;
    const pageChars = HIRAGANA.slice(start, start + CHARS_PER_PAGE);
    chars.push(...pageChars.slice(0, EMPTY_REVIEW_INJECT_CHARS_PER_PAGE));
  }
  return chars;
}

test.describe('グリフ化できない採用セルの要確認可視化 (#112/#108)', () => {
  test('採用されるがベクター化が空のセルが「要確認」として現れ、黙って欠字しない', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    const files = fs
      .readdirSync(SCANS_DIR)
      .filter((f) => f.endsWith('.png'))
      .sort()
      .map((f) => path.join(SCANS_DIR, f));
    expect(files.length).toBeGreaterThanOrEqual(2);

    const zipPath = path.join(SCANS_DIR, '..', 'test-upload-emptyreview.zip');
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

        // 注入セルが「要確認」として現れる（黙って空グリフに倒さない）
        await expect(page.locator('.scan-grid__cell--review')).toHaveCount(reviewChars.length);
        for (const char of reviewChars) {
          await expect(
            page
              .locator('.scan-grid__cell--review')
              .filter({ has: page.locator('.scan-grid__cell-char', { hasText: char }) }),
            `文字「${char}」が要確認として表示されるべき`,
          ).toHaveCount(1);
        }

        // 段階診断ログ: [scan:cells] に各ページの review>=1 が出ている
        for (let p = 1; p <= files.length; p++) {
          expect(
            logs.some((l) =>
              new RegExp(
                `\\[scan:cells\\] page=${p} .*review=${EMPTY_REVIEW_INJECT_CHARS_PER_PAGE}`,
              ).test(l.text),
            ),
            `[scan:cells] page=${p} の review ログがない`,
          ).toBe(true);
        }
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
