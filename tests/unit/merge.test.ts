import { describe, it, expect } from 'vitest';
import { mergeScanIntoExisting, mergeImportIntoExisting } from '../../src/lib/merge';
import type { GlyphStatus } from '../../src/lib/scanner/processor';
import type { VectorGlyph } from '../../src/lib/font/builder';

function makeStatus(char: string, status: 'found' | 'empty' | 'imported'): GlyphStatus {
  return {
    char,
    unicode: char.codePointAt(0)!,
    pageIndex: 0,
    row: 0,
    col: 0,
    status,
  };
}

function makeGlyph(char: string, alt?: number): VectorGlyph {
  const unicode = char.codePointAt(0)!;
  const hex = unicode.toString(16).toUpperCase().padStart(4, '0');
  return {
    name: alt ? `uni${hex}.alt${alt}` : `uni${hex}`,
    unicode: alt ? undefined : unicode,
    paths: [],
    advanceWidth: 1000,
  };
}

describe('mergeScanIntoExisting', () => {
  it('found が imported を上書きする', () => {
    const prevStatuses = [makeStatus('あ', 'imported'), makeStatus('い', 'imported')];
    const prevGlyphs = [makeGlyph('あ'), makeGlyph('い')];
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [makeGlyph('あ')];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.statuses[0].status).toBe('found');
    expect(result.statuses[1].status).toBe('imported');
    expect(result.glyphs).toHaveLength(2);
  });

  it('found が empty を上書きする', () => {
    const prevStatuses = [makeStatus('あ', 'empty')];
    const prevGlyphs: VectorGlyph[] = [];
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [makeGlyph('あ')];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.statuses[0].status).toBe('found');
    expect(result.glyphs).toHaveLength(1);
  });

  it('found は既存の found を後勝ちで上書きする (Issue #93)', () => {
    const prevStatuses = [makeStatus('あ', 'found')];
    const prevGlyph = makeGlyph('あ');
    prevGlyph.advanceWidth = 800; // 旧
    const prevGlyphs = [prevGlyph];

    const newStatusObj = makeStatus('あ', 'found');
    newStatusObj.pageIndex = 7; // 識別用に値を変える
    const newGlyph = makeGlyph('あ');
    newGlyph.advanceWidth = 1200; // 新
    const newStatuses = [newStatusObj];
    const newGlyphs = [newGlyph];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.statuses[0].status).toBe('found');
    // 後の status オブジェクト（pageIndex=7）に置き換わっている
    expect(result.statuses[0].pageIndex).toBe(7);
    expect(result.glyphs).toHaveLength(1);
    // 新しいグリフが採用されている
    expect(result.glyphs[0].advanceWidth).toBe(1200);
  });

  it('alt-variant もベースグリフが上書きされたら除外する', () => {
    const prevStatuses = [makeStatus('あ', 'imported')];
    const prevGlyphs = [makeGlyph('あ'), makeGlyph('あ', 1)];
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [makeGlyph('あ')];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.glyphs).toHaveLength(1);
    expect(result.glyphs[0].name).toBe('uni3042');
    expect(result.glyphs.find((g) => g.name.includes('.alt'))).toBeUndefined();
  });

  it('画像由来 (found) の alt も後勝ちで上書きされて除外される (Issue #93)', () => {
    // 既存: 画像由来のベース + alt1
    const prevStatuses = [makeStatus('あ', 'found')];
    const prevGlyphs = [makeGlyph('あ'), makeGlyph('あ', 1)];
    // 新スキャン: ベースのみ（alt なし）
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [makeGlyph('あ')];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.glyphs).toHaveLength(1);
    expect(result.glyphs[0].name).toBe('uni3042');
    expect(result.glyphs.find((g) => g.name.includes('.alt'))).toBeUndefined();
  });

  it('新スキャンの alt-variant も後勝ちで採用される (Issue #93 / レビューM2)', () => {
    // 既存: ベースのみ
    const prev = makeGlyph('あ');
    prev.advanceWidth = 800;
    const prevStatuses = [makeStatus('あ', 'found')];
    const prevGlyphs = [prev];
    // 新スキャン: ベース + alt1 + alt2
    const newBase = makeGlyph('あ');
    newBase.advanceWidth = 1500;
    const newAlt1 = makeGlyph('あ', 1);
    const newAlt2 = makeGlyph('あ', 2);
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [newBase, newAlt1, newAlt2];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    // ベース置換 + alt 2件追加 = 計3件
    expect(result.glyphs).toHaveLength(3);
    expect(result.glyphs[0].advanceWidth).toBe(1500);
    expect(result.glyphs.filter((g) => g.name.includes('.alt'))).toHaveLength(2);
  });
});

describe('mergeImportIntoExisting', () => {
  it('empty を imported で埋める', () => {
    const prevStatuses = [makeStatus('あ', 'found'), makeStatus('い', 'empty')];
    const prevGlyphs = [makeGlyph('あ')];
    const importedStatuses = [makeStatus('あ', 'imported'), makeStatus('い', 'imported')];
    const importedGlyphs = [makeGlyph('あ'), makeGlyph('い')];

    const result = mergeImportIntoExisting(
      prevStatuses,
      prevGlyphs,
      importedStatuses,
      importedGlyphs,
    );

    expect(result.statuses[0].status).toBe('found');
    expect(result.statuses[1].status).toBe('imported');
    expect(result.glyphs).toHaveLength(2);
  });

  it('found は imported で上書きされない', () => {
    const prevStatuses = [makeStatus('あ', 'found')];
    const prevGlyphs = [makeGlyph('あ')];
    const importedStatuses = [makeStatus('あ', 'imported')];
    const importedGlyphs = [makeGlyph('あ')];

    const result = mergeImportIntoExisting(
      prevStatuses,
      prevGlyphs,
      importedStatuses,
      importedGlyphs,
    );

    expect(result.statuses[0].status).toBe('found');
    expect(result.glyphs).toHaveLength(1);
  });

  it('二重インポートで重複しない', () => {
    const prevStatuses = [makeStatus('あ', 'imported')];
    const prevGlyphs = [makeGlyph('あ')];
    const importedStatuses = [makeStatus('あ', 'imported')];
    const importedGlyphs = [makeGlyph('あ')];

    const result = mergeImportIntoExisting(
      prevStatuses,
      prevGlyphs,
      importedStatuses,
      importedGlyphs,
    );

    expect(result.statuses).toHaveLength(1);
    expect(result.glyphs).toHaveLength(1);
  });

  it('imported は新しい imported で後勝ち上書きされる (Issue #93 / レビューM1)', () => {
    const prev = makeGlyph('あ');
    prev.advanceWidth = 700;
    const prevStatuses = [makeStatus('あ', 'imported')];
    const prevGlyphs = [prev];

    const next = makeGlyph('あ');
    next.advanceWidth = 1300;
    const importedStatuses = [makeStatus('あ', 'imported')];
    importedStatuses[0].pageIndex = 5;
    const importedGlyphs = [next];

    const result = mergeImportIntoExisting(
      prevStatuses,
      prevGlyphs,
      importedStatuses,
      importedGlyphs,
    );

    expect(result.statuses[0].pageIndex).toBe(5);
    expect(result.glyphs).toHaveLength(1);
    expect(result.glyphs[0].advanceWidth).toBe(1300);
  });
});
