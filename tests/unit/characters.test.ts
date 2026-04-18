import { describe, it, expect } from 'vitest';
import {
  HIRAGANA,
  KATAKANA,
  buildCharListFromSelection,
  selectionToFlag,
  flagToSelection,
  getCharactersForPage,
  getTotalPages,
  CHARS_PER_PAGE,
  type CharSelection,
} from '../../src/data/characters';

const SEL_NONE: CharSelection = {
  hiragana: false,
  katakana: false,
  alphanum: false,
  kanji: false,
};

describe('selectionToFlag', () => {
  it('ひらがなのみ → "h"', () => {
    expect(selectionToFlag({ ...SEL_NONE, hiragana: true })).toBe('h');
  });

  it('ひらがな+カタカナ → "hk"（順序固定）', () => {
    expect(selectionToFlag({ ...SEL_NONE, hiragana: true, katakana: true })).toBe('hk');
  });

  it('全選択 → "hkaj"', () => {
    expect(selectionToFlag({ hiragana: true, katakana: true, alphanum: true, kanji: true })).toBe(
      'hkaj',
    );
  });

  it('非選択順でも h→k→a→j の順に並ぶ', () => {
    expect(selectionToFlag({ ...SEL_NONE, kanji: true, hiragana: true })).toBe('hj');
  });

  it('何も選択していない → 空文字列', () => {
    expect(selectionToFlag(SEL_NONE)).toBe('');
  });
});

describe('flagToSelection', () => {
  it('"h" → ひらがなのみ', () => {
    expect(flagToSelection('h')).toEqual({
      hiragana: true,
      katakana: false,
      alphanum: false,
      kanji: false,
    });
  });

  it('"hkaj" → 全 true', () => {
    expect(flagToSelection('hkaj')).toEqual({
      hiragana: true,
      katakana: true,
      alphanum: true,
      kanji: true,
    });
  });

  it('不正な文字 "xyz" → null', () => {
    expect(flagToSelection('xyz')).toBeNull();
  });

  it('空文字列 → null', () => {
    expect(flagToSelection('')).toBeNull();
  });

  it('有効+無効の混在 → null', () => {
    expect(flagToSelection('hx')).toBeNull();
  });
});

describe('selectionToFlag <-> flagToSelection 往復', () => {
  const patterns: CharSelection[] = [
    { hiragana: true, katakana: false, alphanum: false, kanji: false },
    { hiragana: true, katakana: true, alphanum: false, kanji: false },
    { hiragana: true, katakana: true, alphanum: true, kanji: false },
    { hiragana: true, katakana: true, alphanum: true, kanji: true },
    { hiragana: false, katakana: false, alphanum: true, kanji: false },
  ];
  for (const sel of patterns) {
    it(`${JSON.stringify(sel)} は往復で一致`, () => {
      const flag = selectionToFlag(sel);
      expect(flagToSelection(flag)).toEqual(sel);
    });
  }
});

describe('buildCharListFromSelection', () => {
  it('ひらがなのみ → HIRAGANA 全文字', () => {
    const list = buildCharListFromSelection({ ...SEL_NONE, hiragana: true });
    expect(list).toEqual(HIRAGANA);
  });

  it('ひらがな+カタカナ → HIRAGANA + KATAKANA', () => {
    const list = buildCharListFromSelection({ ...SEL_NONE, hiragana: true, katakana: true });
    expect(list).toEqual([...HIRAGANA, ...KATAKANA]);
  });
});

describe('getCharactersForPage', () => {
  it('ひらがな選択の 0 ページ目は HIRAGANA 先頭から最大 CHARS_PER_PAGE 文字', () => {
    const page0 = getCharactersForPage(0, { ...SEL_NONE, hiragana: true });
    // HIRAGANA.length === 83 で CHARS_PER_PAGE 以下ならそのまま返る
    expect(page0.length).toBe(Math.min(HIRAGANA.length, CHARS_PER_PAGE));
    expect(page0[0]).toBe(HIRAGANA[0]);
  });

  it('最後のページも正しくスライスされる', () => {
    const selection: CharSelection = { ...SEL_NONE, hiragana: true, katakana: true };
    const total = getTotalPages(selection);
    const lastPage = getCharactersForPage(total - 1, selection);
    const all = buildCharListFromSelection(selection);
    const expected = all.slice((total - 1) * CHARS_PER_PAGE);
    expect(lastPage).toEqual(expected);
    expect(lastPage.length).toBeGreaterThan(0);
    expect(lastPage.length).toBeLessThanOrEqual(CHARS_PER_PAGE);
  });

  it('範囲外ページは空配列', () => {
    const selection: CharSelection = { ...SEL_NONE, hiragana: true };
    const total = getTotalPages(selection);
    expect(getCharactersForPage(total, selection)).toEqual([]);
  });
});

describe('getTotalPages', () => {
  it('選択を変えると総ページ数も変わる', () => {
    const hOnly = getTotalPages({ ...SEL_NONE, hiragana: true });
    const all = getTotalPages({ hiragana: true, katakana: true, alphanum: true, kanji: true });
    expect(all).toBeGreaterThan(hOnly);
  });
});
