import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { HIRAGANA } from '../../src/data/characters';
import {
  createZipFromFiles,
  expectGlyphsForChars,
  loadFont,
  runScanToFontFlow,
  withStageLogs,
} from './font-flow-utils';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const MOCK_SCANS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans');

test.describe('フルフロー: テンプレート→スキャン→フォント生成', () => {
  test('テンプレートPDFをダウンロードできる', async ({ page }) => {
    await page.goto('/');

    // テンプレートページへ遷移（ヘッダー nav は <a> リンク化済み、ホーム大ボタンは <button>）
    await page.getByRole('link', { name: '1. テンプレート', exact: true }).click();
    await expect(page.locator('h2')).toContainText('テンプレートを印刷する');

    // ひらがなのみチェック（他を外す）
    const checkboxes = page.locator('.checkbox-group label');

    // カタカナ・英数字・常用漢字のチェックを外す
    const katakanaCheckbox = checkboxes.filter({ hasText: 'カタカナ' }).locator('input');
    const alphaNumCheckbox = checkboxes.filter({ hasText: '英数字' }).locator('input');
    const kanjiCheckbox = checkboxes.filter({ hasText: '常用漢字' }).locator('input');

    if (await katakanaCheckbox.isChecked()) await katakanaCheckbox.uncheck();
    if (await alphaNumCheckbox.isChecked()) await alphaNumCheckbox.uncheck();
    if (await kanjiCheckbox.isChecked()) await kanjiCheckbox.uncheck();

    // ひらがなはチェック済みのはず
    const hiraganaCheckbox = checkboxes.filter({ hasText: 'ひらがな' }).locator('input');
    await expect(hiraganaCheckbox).toBeChecked();

    // PDFダウンロード
    const downloadPromise = page.waitForEvent('download');
    await page.click('text=PDFをダウンロード');
    const download = await downloadPromise;

    expect(download.suggestedFilename()).toBe('MyFontCraft-template.pdf');
    const downloadPath = await download.path();
    expect(downloadPath).toBeTruthy();

    const stat = fs.statSync(downloadPath!);
    // ひらがなのみ（83文字 = 3ページ）なので、数KB以上あるはず
    expect(stat.size).toBeGreaterThan(5000);
  });

  test('模擬スキャン画像をアップロードしてフォントを生成できる', async ({ page }, testInfo) => {
    test.setTimeout(300_000);

    // ZIPファイルを作成（正面画像の全ページ）
    const files = fs
      .readdirSync(MOCK_SCANS_DIR)
      .filter((f) => f.endsWith('.png'))
      .sort()
      .map((f) => path.join(MOCK_SCANS_DIR, f));
    // ひらがな83文字は 2 ページに収まる（generate-mock-scans.ts の出力枚数と一致）
    expect(files.length).toBeGreaterThanOrEqual(2);

    const zipPath = path.join(MOCK_SCANS_DIR, '..', 'test-upload.zip');
    await createZipFromFiles(files, zipPath);

    try {
      const { logs } = await withStageLogs(page, testInfo, async () => {
        const fontPath = await runScanToFontFlow(page, zipPath);

        // TTFファイルサイズ確認
        const stat = fs.statSync(fontPath);
        expect(stat.size).toBeGreaterThan(1000);

        // フィクスチャに描画した全ひらがな83文字について、
        // グリフが存在し（index > 0）かつパスが空でないことを検証する
        const font = loadFont(fontPath);
        expectGlyphsForChars(font, HIRAGANA);
      });

      // 段階診断ログ（#109）: golden path 成功時は最終段階 font-input まで ok で到達し、
      // 途中段階のエラーログ（[scan:*] の console.error）が1件も出ていないこと
      expect(
        logs.some((l) => l.text.startsWith('[scan:font-input] ok')),
        '段階ログに [scan:font-input] ok がない',
      ).toBe(true);
      const scanErrors = logs.filter((l) => l.type === 'error' && l.text.startsWith('[scan:'));
      expect(scanErrors, '成功パスなのに [scan:*] エラーログがある').toEqual([]);
    } finally {
      // テスト用ZIPを削除
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
