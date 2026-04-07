import type { GlyphStatus } from './scanner/processor';
import type { VectorGlyph } from './font/builder';

/**
 * スキャン結果を既存のステータス/グリフにマージする
 * found が imported / empty を上書き（スキャンが優先）
 */
export function mergeScanIntoExisting(
  prevStatuses: GlyphStatus[],
  prevGlyphs: VectorGlyph[],
  newStatuses: GlyphStatus[],
  newGlyphs: VectorGlyph[],
): { statuses: GlyphStatus[]; glyphs: VectorGlyph[] } {
  const newFound = new Map<number, GlyphStatus>();
  for (const gs of newStatuses) {
    if (gs.status === 'found') newFound.set(gs.unicode, gs);
  }

  const statuses = prevStatuses.map((gs) => {
    if ((gs.status === 'empty' || gs.status === 'imported') && newFound.has(gs.unicode)) {
      return newFound.get(gs.unicode)!;
    }
    return gs;
  });

  const newFoundUnicodes = new Set(newFound.keys());
  // alt-variant もベースグリフが上書きされたら除外
  const keptGlyphs = prevGlyphs.filter((g) => {
    if (g.unicode) return !newFoundUnicodes.has(g.unicode);
    const baseUnicode = parseInt(g.name.replace(/^uni/, '').replace(/\.alt\d+$/, ''), 16);
    return isNaN(baseUnicode) || !newFoundUnicodes.has(baseUnicode);
  });

  const existingKeptUnicodes = new Set(keptGlyphs.map((g) => g.unicode));
  const addedGlyphs = newGlyphs.filter((g) => g.unicode && !existingKeptUnicodes.has(g.unicode));
  const glyphs = [...keptGlyphs, ...addedGlyphs];

  return { statuses, glyphs };
}

/**
 * インポート結果を既存のステータス/グリフにマージする
 * empty のみを imported で埋める（found は上書きしない）
 */
export function mergeImportIntoExisting(
  prevStatuses: GlyphStatus[],
  prevGlyphs: VectorGlyph[],
  importedStatuses: GlyphStatus[],
  importedGlyphs: VectorGlyph[],
): { statuses: GlyphStatus[]; glyphs: VectorGlyph[] } {
  const importedMap = new Map<number, { glyph: VectorGlyph; status: GlyphStatus }>();
  for (let i = 0; i < importedGlyphs.length; i++) {
    importedMap.set(importedStatuses[i].unicode, {
      glyph: importedGlyphs[i],
      status: importedStatuses[i],
    });
  }

  const statuses = prevStatuses.map((gs) => {
    if (gs.status === 'empty' && importedMap.has(gs.unicode)) {
      return importedMap.get(gs.unicode)!.status;
    }
    return gs;
  });

  const existingUnicodes = new Set(prevGlyphs.map((g) => g.unicode));
  const addedGlyphs = importedGlyphs.filter((g) => g.unicode && !existingUnicodes.has(g.unicode));
  const glyphs = [...prevGlyphs, ...addedGlyphs];

  return { statuses, glyphs };
}
