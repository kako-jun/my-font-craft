/**
 * フォントビルダーのユニットテスト
 * 合成グリフデータから .ttf を生成し、opentype.js で読み返して検証
 */
import { describe, it, expect } from 'vitest';
import '../helpers/canvas-polyfill';
import opentype from 'opentype.js';
import { buildFont, importFont, type VectorGlyph } from '../../src/lib/font/builder';

describe('Font Builder', () => {
  it('should build a valid TTF with test glyphs', async () => {
    const testGlyphs: VectorGlyph[] = [
      {
        name: 'uni3042', // あ
        unicode: 0x3042,
        advanceWidth: 1000,
        paths: [
          [
            { type: 'M', x: 200, y: 200 },
            { type: 'L', x: 800, y: 200 },
            { type: 'L', x: 800, y: 700 },
            { type: 'L', x: 200, y: 700 },
            { type: 'Z', x: 200, y: 200 },
          ],
        ],
      },
      {
        name: 'uni3044', // い
        unicode: 0x3044,
        advanceWidth: 1000,
        paths: [
          [
            { type: 'M', x: 300, y: 100 },
            { type: 'L', x: 700, y: 100 },
            { type: 'L', x: 700, y: 800 },
            { type: 'L', x: 300, y: 800 },
            { type: 'Z', x: 300, y: 100 },
          ],
        ],
      },
    ];

    const buffer = await buildFont({
      familyName: 'TestFont',
      glyphs: testGlyphs,
    });

    expect(buffer).toBeInstanceOf(ArrayBuffer);
    expect(buffer.byteLength).toBeGreaterThan(0);

    // opentype.js で読み返す
    const font = opentype.parse(buffer);
    expect(font.names.fontFamily?.en).toBe('TestFont');
    expect(font.unitsPerEm).toBe(1000);

    // .notdef + space + 2 test glyphs = 4
    expect(font.glyphs.length).toBe(4);

    // Unicode マッピングの確認
    const glyphA = font.charToGlyph('あ');
    expect(glyphA).toBeDefined();
    expect(glyphA.name).toBe('uni3042');

    const glyphI = font.charToGlyph('い');
    expect(glyphI).toBeDefined();
    expect(glyphI.name).toBe('uni3044');
  });

  it('should round-trip: build then import', async () => {
    const testGlyphs: VectorGlyph[] = [
      {
        name: 'uni3042', // あ
        unicode: 0x3042,
        advanceWidth: 1000,
        paths: [
          [
            { type: 'M', x: 200, y: 200 },
            { type: 'L', x: 800, y: 200 },
            { type: 'L', x: 800, y: 700 },
            { type: 'L', x: 200, y: 700 },
            { type: 'Z', x: 0, y: 0 },
          ],
        ],
      },
      {
        name: 'uni3044', // い
        unicode: 0x3044,
        advanceWidth: 1000,
        paths: [
          [
            { type: 'M', x: 300, y: 100 },
            { type: 'C', x: 700, y: 100, cp1x: 400, cp1y: 50, cp2x: 600, cp2y: 50 },
            { type: 'L', x: 700, y: 800 },
            { type: 'L', x: 300, y: 800 },
            { type: 'Z', x: 0, y: 0 },
          ],
        ],
      },
    ];

    const buffer = await buildFont({
      familyName: 'RoundTripTest',
      glyphs: testGlyphs,
    });

    const result = importFont(buffer);

    // .notdef と space はスキップされる
    expect(result.glyphs.length).toBe(2);
    expect(result.statuses.length).toBe(2);

    // ステータスが imported であること
    expect(result.statuses[0].status).toBe('imported');
    expect(result.statuses[1].status).toBe('imported');

    // Unicode が正しく取得されていること
    const unicodes = result.glyphs.map((g) => g.unicode).sort();
    expect(unicodes).toEqual([0x3042, 0x3044]);

    // 文字が正しく取得されていること
    const chars = result.statuses.map((s) => s.char).sort();
    expect(chars).toEqual(['あ', 'い']);

    // パスが空でないこと
    for (const glyph of result.glyphs) {
      expect(glyph.paths.length).toBeGreaterThan(0);
      expect(glyph.paths[0].length).toBeGreaterThan(0);
    }

    // advanceWidth が保持されること
    for (const glyph of result.glyphs) {
      expect(glyph.advanceWidth).toBe(1000);
    }
  });

  it('should round-trip a holed glyph with a C curve (外輪郭+穴, #112)', async () => {
    // 穴あきグリフ（アニュラス）: 外輪郭 CW（大きい正方・C 曲線を含む）+ 穴 CCW（内側正方）。
    // CFF 書き出し → import で 2輪郭・曲線が保持され、穴が別サブパスとして残ることを確認。
    const holed: VectorGlyph[] = [
      {
        name: 'uni25A1', // □（穴あき代理）
        unicode: 0x25a1,
        advanceWidth: 1000,
        paths: [
          // 外輪郭（CW, 反時計回りでない）: 上辺を C 曲線にする
          [
            { type: 'M', x: 100, y: 100 },
            { type: 'C', x: 900, y: 100, cp1x: 400, cp1y: 50, cp2x: 600, cp2y: 50 },
            { type: 'L', x: 900, y: 900 },
            { type: 'L', x: 100, y: 900 },
            { type: 'Z', x: 100, y: 100 },
          ],
          // 穴（CCW）
          [
            { type: 'M', x: 300, y: 300 },
            { type: 'L', x: 300, y: 700 },
            { type: 'L', x: 700, y: 700 },
            { type: 'L', x: 700, y: 300 },
            { type: 'Z', x: 300, y: 300 },
          ],
        ],
      },
    ];

    const buffer = await buildFont({ familyName: 'HoledFont', glyphs: holed });
    const result = importFont(buffer);

    expect(result.glyphs.length).toBe(1);
    const g = result.glyphs[0];
    expect(g.unicode).toBe(0x25a1);
    // 2輪郭（外輪郭 + 穴）が保持される
    expect(g.paths.length).toBe(2);
    // 曲線コマンド（C）が往復後も残る（CFF は cubic ネイティブ）
    const allCmds = g.paths.flat();
    expect(allCmds.some((c) => c.type === 'C')).toBe(true);
    // 各サブパスが閉じている（Z を含む）
    for (const sub of g.paths) {
      expect(sub.some((c) => c.type === 'Z')).toBe(true);
    }
  });

  it('should skip empty-paths glyphs to preserve OS/browser fallback (#117)', async () => {
    const glyphs: VectorGlyph[] = [
      {
        name: 'uni3042', // あ（正常な輪郭あり）
        unicode: 0x3042,
        advanceWidth: 1000,
        paths: [
          [
            { type: 'M', x: 200, y: 200 },
            { type: 'L', x: 800, y: 200 },
            { type: 'L', x: 800, y: 700 },
            { type: 'L', x: 200, y: 700 },
            { type: 'Z', x: 0, y: 0 },
          ],
        ],
      },
      {
        name: 'uni3044', // い（残渣がゼロ化され paths が空 = 描画コマンド無し）
        unicode: 0x3044,
        advanceWidth: 1000,
        paths: [],
      },
    ];

    const buffer = await buildFont({ familyName: 'FallbackFont', glyphs });
    const font = opentype.parse(buffer);

    // 正常グリフのコードポイントは存在する
    const glyphA = font.charToGlyph('あ');
    expect(glyphA.name).toBe('uni3042');
    expect(font.charToGlyph('あ').index).not.toBe(0);

    // 空グリフのコードポイントは未割当（.notdef=index 0 に落ちる）→ フォールバック温存
    expect(font.charToGlyph('い').index).toBe(0);

    // グリフ数: .notdef + space + あ のみ（い はスキップ）
    expect(font.glyphs.length).toBe(3);
  });

  it('should skip .notdef and space when importing', async () => {
    // 空のフォント（.notdef + space のみ）
    const buffer = await buildFont({
      familyName: 'EmptyImport',
      glyphs: [],
    });

    const result = importFont(buffer);
    expect(result.glyphs.length).toBe(0);
    expect(result.statuses.length).toBe(0);
  });

  it('should include .notdef and space glyphs', async () => {
    const buffer = await buildFont({
      familyName: 'EmptyFont',
      glyphs: [],
    });

    const font = opentype.parse(buffer);
    expect(font.glyphs.length).toBe(2); // .notdef + space

    const space = font.charToGlyph(' ');
    expect(space.name).toBe('space');
    expect(space.advanceWidth).toBe(500);
  });
});
