/**
 * セル→em 固定変換（#111）の配置検証 e2e。
 *
 * metrics フィクスチャページ（QR は #96 リトライPDF形式の chars ペイロード）に
 * 句読点・小書きかな・descender 文字をフォント描画し、ひらがな2ページと合わせて
 * スキャン→フォント生成する。生成 TTF のグリフ bbox で「書いた位置・大きさが
 * そのままフォントに出る」ことを検証する:
 *   (a) 小書きかな「っ」「ぁ」が通常「つ」「あ」より小さい（等倍化されない）
 *   (b) 句読点「、」「。」がベースライン付近の低い位置に小さく乗る（中央に浮かない）
 *   (c) g/j/p/q/y の descender が y<0（ベースライン下）に届く
 * 最後に生成フォントで文章を実描画してスクリーンショットを残す。
 *
 * 期待値のマージンは CLI パイプライン実測に基づく（Issue #111 記録）:
 *   、 x[61,272] y[-43,186] ／ 。 x[69,331] y[-51,228]
 *   っ 465×363 vs つ 574×448 ／ ぁ 448×507 vs あ 549×625
 *   g/j/p/q/y の yMin -51〜-59 ／ ー 609×50
 */

import { test, expect } from '@playwright/test';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { METRICS_PAGE_CHARS } from '../fixtures/generate-mock-scans';
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
const METRICS_DIR = path.join(__dirname, '..', 'fixtures', 'mock-scans-metrics');

test.describe('セル→em 固定変換 (#111)', () => {
  test('句読点・小書きかな・descender が書いた位置と大きさでフォントに出る', async ({
    page,
  }, testInfo) => {
    test.setTimeout(300_000);

    // ひらがな2ページ + metrics ページを1つの ZIP にまとめてアップロードする
    const files = [
      ...fs
        .readdirSync(MOCK_SCANS_DIR)
        .filter((f) => f.endsWith('.png'))
        .sort()
        .map((f) => path.join(MOCK_SCANS_DIR, f)),
      ...fs
        .readdirSync(METRICS_DIR)
        .filter((f) => f.endsWith('.png'))
        .sort()
        .map((f) => path.join(METRICS_DIR, f)),
    ];
    expect(files.length).toBeGreaterThanOrEqual(3);

    const zipPath = path.join(METRICS_DIR, '..', 'test-upload-metrics.zip');
    await createZipFromFiles(files, zipPath);

    try {
      await withStageLogs(page, testInfo, async () => {
        const fontPath = await runScanToFontFlow(page, zipPath);
        const font = loadFont(fontPath);
        expectGlyphsForChars(font, METRICS_PAGE_CHARS);

        const bbox = (ch: string) => font.charToGlyph(ch).path.getBoundingBox();
        const width = (b: { x1: number; x2: number }) => b.x2 - b.x1;
        const height = (b: { y1: number; y2: number }) => b.y2 - b.y1;

        // (a) 小書きかなは通常かなより小さいまま（bbox 正規化の等倍化が起きない）
        const tsu = bbox('つ');
        const smallTsu = bbox('っ');
        expect(width(smallTsu), 'っ の幅 < つ の幅').toBeLessThan(width(tsu));
        expect(height(smallTsu), 'っ の高さ < つ の高さ').toBeLessThan(height(tsu));
        expect(width(smallTsu) * height(smallTsu), 'っ の bbox 面積は つ の 85% 未満').toBeLessThan(
          width(tsu) * height(tsu) * 0.85,
        );

        const a = bbox('あ');
        const smallA = bbox('ぁ');
        expect(width(smallA), 'ぁ の幅 < あ の幅').toBeLessThan(width(a));
        expect(height(smallA), 'ぁ の高さ < あ の高さ').toBeLessThan(height(a));

        // サニティ: 通常かなは十分大きく写っている（フィクスチャ自体の退行検知）
        expect(height(a), 'あ の高さは 450 units 超').toBeGreaterThan(450);

        // (b) 句読点はベースライン付近の低い位置に小さく（em 中央 y≈380 に浮かない）
        for (const ch of ['、', '。']) {
          const b = bbox(ch);
          expect(b.y2, `${ch} の上端はベースライン付近（y2 < 350）`).toBeLessThan(350);
          expect(height(b), `${ch} の高さは小さい（< 400）`).toBeLessThan(400);
          expect(width(b), `${ch} の幅は小さい（< 400）`).toBeLessThan(400);
          expect(b.x1, `${ch} は左寄り（x1 < 350）`).toBeLessThan(350);
        }

        // (c) descender: g/j/p/q/y のインクがベースライン下（y<0）に届く
        for (const ch of ['g', 'j', 'p', 'q', 'y']) {
          expect(bbox(ch).y1, `${ch} の下端は y<0（descender 領域）`).toBeLessThan(0);
        }

        // 長音「ー」は横長の細い棒のまま（正方形に拡大されない）
        const bar = bbox('ー');
        expect(height(bar), 'ー の高さは細い（< 150）').toBeLessThan(150);
        expect(width(bar), 'ー の幅は長い（> 450）').toBeGreaterThan(450);

        // 生成フォントで実際に文章を描画してスクリーンショットを残す
        const fontB64 = fs.readFileSync(fontPath).toString('base64');
        await page.setContent(`<!doctype html><html><head><meta charset="utf-8"><style>
          @font-face {
            font-family: 'MFCGenerated';
            src: url(data:font/ttf;base64,${fontB64}) format('truetype');
          }
          body { margin: 40px; background: #ffffff; }
          p {
            font-family: 'MFCGenerated';
            font-size: 72px;
            line-height: 1.5;
            margin: 0 0 16px;
          }
        </style></head><body>
          <p>こんにちは、せかい。</p>
          <p>gjpqy っゃゅょー</p>
        </body></html>`);
        await page.evaluate(() => document.fonts.ready);
        const shotPath = path.join(__dirname, '..', '..', 'test-results', 'font-render-sample.png');
        await page.screenshot({ path: shotPath });
        await testInfo.attach('font-render-sample', { path: shotPath, contentType: 'image/png' });
      });
    } finally {
      if (fs.existsSync(zipPath)) fs.unlinkSync(zipPath);
    }
  });
});
