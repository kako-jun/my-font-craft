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

  it('found は既存の found を上書きしない', () => {
    const prevStatuses = [makeStatus('あ', 'found')];
    const prevGlyphs = [makeGlyph('あ')];
    const newStatuses = [makeStatus('あ', 'found')];
    const newGlyphs = [makeGlyph('あ')];

    const result = mergeScanIntoExisting(prevStatuses, prevGlyphs, newStatuses, newGlyphs);

    expect(result.statuses[0].status).toBe('found');
    // 既存の found が保持される（新しいスキャンで上書き）
    expect(result.glyphs).toHaveLength(1);
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
});
