import type { VectorGlyph } from './builder';

export interface CaltRule {
  baseGlyphName: string;
  altGlyphName: string;
  unicode: number;
}

// calt 機能用のルールを生成
// 同じ文字が連続したとき、交互にバリエーションを使う
export function generateCaltFeature(glyphs: VectorGlyph[]): CaltRule[] {
  const rules: CaltRule[] = [];

  // unicodeを持つグリフをベースとして、.alt を持つものを収集
  const baseGlyphs = new Map<number, string>();
  const altMap = new Map<string, string[]>();

  for (const g of glyphs) {
    if (g.unicode && !g.name.includes('.alt')) {
      baseGlyphs.set(g.unicode, g.name);
    }
    if (g.name.includes('.alt')) {
      const baseName = g.name.split('.')[0];
      const alts = altMap.get(baseName) || [];
      alts.push(g.name);
      altMap.set(baseName, alts);
    }
  }

  // 各ベースグリフに対してルールを生成
  for (const [unicode, baseName] of baseGlyphs) {
    const alts = altMap.get(baseName);
    if (!alts || alts.length === 0) continue;

    // 基本: ベースの後にベースが来たら → alt1 に置換
    rules.push({
      baseGlyphName: baseName,
      altGlyphName: alts[0],
      unicode,
    });
  }

  return rules;
}
