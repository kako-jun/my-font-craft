/**
 * e2e 共通ユーティリティ: ZIP アップロード → スキャン → フォント生成 → TTF 検証（#109）
 *
 * full-flow.spec.ts（正面画像）と distorted-flow.spec.ts（歪み画像）で共用する。
 */

import { expect, type Page, type TestInfo } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import JSZip from 'jszip';
import opentype from 'opentype.js';

/** 画像ファイル群を ZIP にまとめて zipPath に書き出す */
export async function createZipFromFiles(filePaths: string[], zipPath: string): Promise<void> {
  const zip = new JSZip();
  for (const filePath of filePaths) {
    zip.file(path.basename(filePath), fs.readFileSync(filePath));
  }
  const buf = await zip.generateAsync({ type: 'nodebuffer' });
  fs.writeFileSync(zipPath, buf);
}

/** 収集した段階診断ログ1行。type は console のレベル（'error' / 'info' など） */
export interface StageLogEntry {
  type: string;
  text: string;
}

/**
 * 段階診断ログを収集しながら fn を実行する。
 * 失敗時は `[scan:*]`（processor.ts）と `=== ステップN ===`（WASM 内部）の
 * ログをテスト添付 + コンソールに出力し、どの段階で落ちたか特定できるようにする。
 * fn には収集中のログ配列を渡す。段階ログ自体の検証は fn 内で行うこと
 * （fn 内での失敗なら scan-stage-logs 添付が付く）。成功時も収集済みログを返す。
 */
export async function withStageLogs<T>(
  page: Page,
  testInfo: TestInfo,
  fn: (logs: StageLogEntry[]) => Promise<T>,
): Promise<{ result: T; logs: StageLogEntry[] }> {
  const logs: StageLogEntry[] = [];
  const onConsole = (msg: { type(): string; text(): string }) => {
    const text = msg.text();
    if (text.startsWith('[scan:') || text.startsWith('===')) {
      logs.push({ type: msg.type(), text });
    }
  };
  page.on('console', onConsole);
  try {
    const result = await fn(logs);
    return { result, logs };
  } catch (e) {
    await testInfo.attach('scan-stage-logs', {
      body: logs.length > 0 ? logs.map((l) => l.text).join('\n') : '(段階ログなし)',
      contentType: 'text/plain',
    });
    console.error('--- scan stage logs (tail) ---');
    for (const { text } of logs.slice(-60)) console.error(text);
    throw e;
  } finally {
    // 同一 page で複数回使ったときに listener が残って二重収集になるのを防ぐ
    page.off('console', onConsole);
  }
}

/**
 * フォント作成ページで ZIP をアップロードし、スキャン → フォント生成 → TTF
 * ダウンロードまで実行して、ダウンロードされた TTF のパスを返す。
 */
export async function runScanToFontFlow(page: Page, zipPath: string): Promise<string> {
  await page.goto('/');

  // フォント作成ページへ遷移（#98: ヘッダー nav は <a> リンク化済み）
  await page.getByRole('link', { name: '2. フォント作成', exact: true }).click();
  await expect(page.locator('h2')).toContainText('フォントを作成する');

  // ZIPをアップロード（hidden input に直接セット）
  await page.locator('#zip-input').setInputFiles(zipPath);

  // スキャン処理の完了を待つ（review フェーズのボタンが出る）
  await expect(page.locator('button', { hasText: /フォントを生成|このまま生成/ })).toBeVisible({
    timeout: 180_000,
  });

  await page.click('button:has-text("フォントを生成"), button:has-text("このまま生成")');

  // フォント生成完了を待つ
  await expect(page.locator('text=フォントが完成しました')).toBeVisible({ timeout: 90_000 });

  // TTFダウンロード
  const downloadPromise = page.waitForEvent('download');
  await page.click('text=フォントをダウンロード');
  const download = await downloadPromise;

  expect(download.suggestedFilename()).toMatch(/\.ttf$/);
  const downloadPath = await download.path();
  expect(downloadPath).toBeTruthy();
  return downloadPath!;
}

/** TTF ファイルを opentype.js で読み込む */
export function loadFont(fontPath: string): opentype.Font {
  const fontBuffer = fs.readFileSync(fontPath);
  const arrayBuffer = fontBuffer.buffer.slice(
    fontBuffer.byteOffset,
    fontBuffer.byteOffset + fontBuffer.byteLength,
  );
  return opentype.parse(arrayBuffer);
}

/**
 * 期待文字の全てについて、グリフが存在し（.notdef でない）かつ
 * パスが空でないことを検証する。
 */
export function expectGlyphsForChars(font: opentype.Font, chars: string[]): void {
  const missing: string[] = [];
  const emptyPath: string[] = [];
  for (const char of chars) {
    const glyph = font.charToGlyph(char);
    if (!glyph || glyph.index === 0) {
      missing.push(char);
      continue;
    }
    if (!glyph.path || glyph.path.commands.length === 0) {
      emptyPath.push(char);
    }
  }
  expect(missing, `グリフが見つからない文字: ${missing.join('')}`).toEqual([]);
  expect(emptyPath, `パスが空の文字: ${emptyPath.join('')}`).toEqual([]);
}
