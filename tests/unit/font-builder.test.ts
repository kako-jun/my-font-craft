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
