import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import JSZip from 'jszip';
import opentype from 'opentype.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const MOCK_SCANS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans');

/**
 * 模擬スキャン画像3枚をZIPにまとめる
 */
async function createMockZip(): Promise<Buffer> {
  const zip = new JSZip();
  const files = fs.readdirSync(MOCK_SCANS_DIR).filter((f) => f.endsWith('.png'));
  expect(files.length).toBeGreaterThanOrEqual(3);

  for (const file of files) {
    const data = fs.readFileSync(path.join(MOCK_SCANS_DIR, file));
    zip.file(file, data);
  }

  const buf = await zip.generateAsync({ type: 'nodebuffer' });
  return buf;
}

test.describe('フルフロー: テンプレート→スキャン→フォント生成', () => {
  test('テンプレートPDFをダウンロードできる', async ({ page }) => {
    await page.goto('/');

    // テンプレートページへ遷移
    await page.click('text=1. テンプレート');
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

  test('模擬スキャン画像をアップロードしてフォントを生成できる', async ({ page }) => {
    await page.goto('/');

    // フォント作成ページへ遷移
    await page.click('text=2. フォント作成');
    await expect(page.locator('h2')).toContainText('フォントを作成する');

    // ZIPファイルを作成
    const zipBuffer = await createMockZip();
    const zipPath = path.join(MOCK_SCANS_DIR, '..', 'test-upload.zip');
    fs.writeFileSync(zipPath, zipBuffer);

    try {
      // ZIPをアップロード（hidden input に直接セット）
      const fileInput = page.locator('#file-input');
      await fileInput.setInputFiles(zipPath);

      // スキャン処理の完了を待つ（review フェーズ）
      // プログレスバーが表示され、その後 review フェーズのボタンが出る
      await expect(page.locator('button', { hasText: /フォントを生成|このまま生成/ })).toBeVisible({
        timeout: 90_000,
      });

      // 「フォントを生成する」または「このまま生成する」ボタンをクリック
      await page.click('button:has-text("フォントを生成"), button:has-text("このまま生成")');

      // フォント生成完了を待つ
      await expect(page.locator('text=フォントが完成しました')).toBeVisible({
        timeout: 90_000,
      });

      // TTFダウンロード
      const downloadPromise = page.waitForEvent('download');
      await page.click('text=フォントをダウンロード');
      const download = await downloadPromise;

      expect(download.suggestedFilename()).toMatch(/\.ttf$/);
      const downloadPath = await download.path();
      expect(downloadPath).toBeTruthy();

      // TTFファイルサイズ確認
      const stat = fs.statSync(downloadPath!);
      expect(stat.size).toBeGreaterThan(1000);

      // opentype.js でフォントを読み込み、ひらがなグリフを検証
      const fontBuffer = fs.readFileSync(downloadPath!);
      const arrayBuffer = fontBuffer.buffer.slice(
        fontBuffer.byteOffset,
        fontBuffer.byteOffset + fontBuffer.byteLength,
      );
      const font = opentype.parse(arrayBuffer);

      // ひらがな「あ」のグリフが存在するか
      const glyphA = font.charToGlyph('あ');
      expect(glyphA).toBeTruthy();
      // .notdef (index 0) でなければ実際のグリフがある
      expect(glyphA.index).toBeGreaterThan(0);

      // 複数のひらがなを検証
      const testChars = ['あ', 'い', 'う', 'か', 'さ'];
      for (const char of testChars) {
        const glyph = font.charToGlyph(char);
        expect(glyph.index, `グリフが見つからない: ${char}`).toBeGreaterThan(0);
      }
    } finally {
      // テスト用ZIPを削除
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
