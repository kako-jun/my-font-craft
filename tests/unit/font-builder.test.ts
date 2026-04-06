/**
 * フォントビルダーのユニットテスト
 * 合成グリフデータから .ttf を生成し、opentype.js で読み返して検証
 */
import { describe, it, expect } from 'vitest';
import '../helpers/canvas-polyfill';
import opentype from 'opentype.js';
import { buildFont, type VectorGlyph } from '../../src/lib/font/builder';

describe('Font Builder', () => {
  it('should build a valid TTF with test glyphs', async () => {
    const testGlyphs: VectorGlyph[] = [
      {
        name: 'uni3042', // あ
        unicode: 0x3042,
        advanceWidth: 1000,
        paths: [[
          { type: 'M', x: 200, y: 200 },
          { type: 'L', x: 800, y: 200 },
          { type: 'L', x: 800, y: 700 },
          { type: 'L', x: 200, y: 700 },
          { type: 'Z', x: 200, y: 200 },
        ]],
      },
      {
        name: 'uni3044', // い
        unicode: 0x3044,
        advanceWidth: 1000,
        paths: [[
          { type: 'M', x: 300, y: 100 },
          { type: 'L', x: 700, y: 100 },
          { type: 'L', x: 700, y: 800 },
          { type: 'L', x: 300, y: 800 },
          { type: 'Z', x: 300, y: 100 },
        ]],
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
