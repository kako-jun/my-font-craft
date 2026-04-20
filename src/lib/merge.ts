import type { GlyphStatus } from './scanner/processor';
import type { VectorGlyph } from './font/builder';

/**
 * スキャン結果を既存のステータス/グリフにマージする
 * 画像由来 (found) は後勝ち: 新しいスキャンは既存の found / imported / empty を全て上書きする
 * （Issue #93: マージ仕様 — 画像は後勝ち / TTFは既存非上書き）
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
    // 画像由来は後勝ち: 既存の status を問わず新しい found で上書きする
    if (newFound.has(gs.unicode)) {
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

  // 新しい glyph はベースも alt-variant も全て採用する。
  // newFound に含まれる unicode に紐づくものだけ通せば、processor が
  // 「同じ unicode のベース + alt 群」を一塊で吐き出す前提で安全。
  const addedGlyphs = newGlyphs.filter((g) => {
    if (g.unicode) return newFoundUnicodes.has(g.unicode);
    const baseUnicode = parseInt(g.name.replace(/^uni/, '').replace(/\.alt\d+$/, ''), 16);
    return !isNaN(baseUnicode) && newFoundUnicodes.has(baseUnicode);
  });
  const glyphs = [...keptGlyphs, ...addedGlyphs];

  return { statuses, glyphs };
}

/**
 * インポート結果を既存のステータス/グリフにマージする
 * - empty / imported を新しい imported で置き換える（TTF同士は後勝ち）
 * - found（画像由来）は守る（TTFは画像由来を上書きしない）
 * （Issue #93: マージ仕様）
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

  // 画像由来 (found) の unicode を保護する
  const protectedUnicodes = new Set<number>();
  for (const gs of prevStatuses) {
    if (gs.status === 'found') protectedUnicodes.add(gs.unicode);
  }

  const statuses = prevStatuses.map((gs) => {
    // found は守る、empty / imported は新しい imported で置き換える（後勝ち）
    if (gs.status !== 'found' && importedMap.has(gs.unicode)) {
      return importedMap.get(gs.unicode)!.status;
    }
    return gs;
  });

  // 既存 glyphs から、画像非保護かつ imported に同 unicode が来ているものを除外
  // （これにより TTF→TTF が後勝ちで置換される）
  const keptGlyphs = prevGlyphs.filter((g) => {
    if (!g.unicode) return true; // alt-variant は判定保留（importFont は alt を作らないため実質発生しない）
    if (protectedUnicodes.has(g.unicode)) return true;
    return !importedMap.has(g.unicode);
  });

  const keptUnicodes = new Set(keptGlyphs.map((g) => g.unicode));
  // 画像由来で守られている unicode は新規 import からも弾く
  const addedGlyphs = importedGlyphs.filter(
    (g) => g.unicode && !protectedUnicodes.has(g.unicode) && !keptUnicodes.has(g.unicode),
  );
  const glyphs = [...keptGlyphs, ...addedGlyphs];

  return { statuses, glyphs };
}
