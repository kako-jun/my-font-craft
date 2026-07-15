/**
 * 失敗系スキャン e2e（#109）
 *
 * マーカーの無い完全に真っ白な A4 相当画像をアップロードし、
 * - UI にユーザー向けエラーメッセージ（撮影ガイド付き）が表示されること
 * - 段階診断ログに `[scan:marker]` のエラー行が出ること（inferFailedStage の実配線）
 * を検証する。マーカー検出はパイプラインの早い段階なので軽量に落ちる。
 *
 * この spec は2つの失敗モードを検証する:
 *
 * 1. ゼロブロブ（真っ白な画像）: 探索領域にブロブが1つも無く、
 *    「TopLeft マーカーが検出できませんでした（ブロブ数=0, フィルタ通過=0）」で決定論的に落ちる。
 *
 * 2. マーカー欠落＋印刷内容あり（#115・修正済み）: 四隅マーカーだけを白塗りし、
 *    タイトル・マス・文字・QR は残した画像。以前は marker.rs が探索領域に残る別ブロブ
 *    （タイトル文字・罫線角）をマーカーと誤検出してパイプラインが先へ進み、QR 段階の
 *    「QRコードを読み取れませんでした（不鮮明）」に化けていた（誤診断）。
 *    検出後クアッド幾何検証（validate_marker_quad, #115）を追加し、組み上がった四角形の
 *    幾何破綻（対辺比・アスペクト・退化）を marker 段階で棄却するようにした。
 *    フィクスチャは tests/fixtures/mock-scans-marker-missing/（generateMarkerMissingScans）。
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCanvas } from 'canvas';
import { createZipFromFiles, withStageLogs } from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const FIXTURES_DIR = path.join(__dirname, '..', 'fixtures');

/** 完全に真っ白な A4 相当（300dpi: 2480x3508）の PNG を作る */
function createBlankScan(outPath: string): void {
  const canvas = createCanvas(2480, 3508);
  const ctx = canvas.getContext('2d');
  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  fs.writeFileSync(outPath, canvas.toBuffer('image/png'));
}

test.describe('失敗系スキャン: マーカー欠落', () => {
  test('マーカー欠落画像でUIエラー表示と [scan:marker] エラーログが出る', async ({
    page,
  }, testInfo) => {
    const blankPng = path.join(FIXTURES_DIR, 'test-blank-page.png');
    const zipPath = path.join(FIXTURES_DIR, 'test-upload-blank.zip');
    createBlankScan(blankPng);
    await createZipFromFiles([blankPng], zipPath);

    try {
      await withStageLogs(page, testInfo, async (logs) => {
        await page.goto('/');
        await page.getByRole('link', { name: '2. フォント作成', exact: true }).click();
        await expect(page.locator('h2')).toContainText('フォントを作成する');

        await page.locator('#zip-input').setInputFiles(zipPath);

        // UI にユーザー向けエラーメッセージ（translateWasmError の撮影ガイド）が出る。
        // timeout は test timeout（120s）より十分短くし、遅い環境でも expect 側の
        // 明確なメッセージで落ちるようにする（実測 2〜3 秒で表示される）
        await expect(page.locator('.message--error')).toContainText(
          'マーカーを検出できませんでした',
          { timeout: 30_000 },
        );

        // 段階診断ログ: marker 段階のエラーとして console.error に出ている。
        // fn 内で検証することで、この検証だけが失敗した場合も scan-stage-logs 添付が付く
        const markerErrors = logs.filter(
          (l) => l.type === 'error' && l.text.startsWith('[scan:marker]'),
        );
        expect(markerErrors.length, '[scan:marker] エラーログがない').toBeGreaterThan(0);
      });
    } finally {
      if (fs.existsSync(blankPng)) fs.unlinkSync(blankPng);
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});

test.describe('失敗系スキャン: マーカー欠落＋印刷内容あり（#115）', () => {
  test('四隅マーカー欠落でQR誤診断に化けず marker 段階で棄却される', async ({ page }, testInfo) => {
    // test:generate-fixtures が生成する。四隅マーカーだけ欠落・他の印刷内容は全て在る。
    // page-01 は探索領域に別ブロブが残り、以前は誤検出→QR不鮮明に化けていたケース。
    const markerMissingPng = path.join(
      FIXTURES_DIR,
      'mock-scans-marker-missing',
      'page-01-marker-missing.png',
    );
    expect(
      fs.existsSync(markerMissingPng),
      'マーカー欠落フィクスチャが無い（npm run test:generate-fixtures 未実行？）',
    ).toBe(true);

    const zipPath = path.join(FIXTURES_DIR, 'test-upload-marker-missing.zip');
    await createZipFromFiles([markerMissingPng], zipPath);

    try {
      await withStageLogs(page, testInfo, async (logs) => {
        await page.goto('/');
        await page.getByRole('link', { name: '2. フォント作成', exact: true }).click();
        await expect(page.locator('h2')).toContainText('フォントを作成する');

        await page.locator('#zip-input').setInputFiles(zipPath);

        // marker 段階のユーザー向けメッセージが出る（撮影ガイド付き・「マーカー」を含む）
        const errorBox = page.locator('.message--error');
        await expect(errorBox).toContainText('マーカー', { timeout: 30_000 });
        // QR 不鮮明への誤診断に化けていない（#115 の回帰ガード）
        await expect(errorBox).not.toContainText('不鮮明');
        await expect(errorBox).not.toContainText('QRコード');

        // 段階診断ログ: marker 段階のエラーとして出ている（perspective/qr ではない）
        const markerErrors = logs.filter(
          (l) => l.type === 'error' && l.text.startsWith('[scan:marker]'),
        );
        expect(markerErrors.length, '[scan:marker] エラーログがない').toBeGreaterThan(0);
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
