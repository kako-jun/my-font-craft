/**
 * 失敗系スキャン e2e（#109）
 *
 * マーカーの無い完全に真っ白な A4 相当画像をアップロードし、
 * - UI にユーザー向けエラーメッセージ（撮影ガイド付き）が表示されること
 * - 段階診断ログに `[scan:marker]` のエラー行が出ること（inferFailedStage の実配線）
 * を検証する。マーカー検出はパイプラインの早い段階なので軽量に落ちる。
 *
 * 注意: 「正面フィクスチャのマーカー円だけ白塗り」方式は使わない。
 * マーカー検出（cli/src/marker.rs）が探索領域内に残る印刷内容（タイトル文字・
 * マスの罫線など）のブロブをマーカーと誤検出してパイプラインが先へ進み、
 * QR 段階のエラー「QRコードを読み取れませんでした」に化けてしまうため（製品側の課題 #115）。
 * 真っ白な画像なら探索領域にブロブが1つも無く、
 * 「TopLeft マーカーが検出できませんでした（ブロブ数=0, フィルタ通過=0）」で決定論的に落ちる。
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
