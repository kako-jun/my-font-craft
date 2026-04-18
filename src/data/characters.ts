import { JOYO_KANJI } from './joyo-kanji';

// ひらがな（83文字）
export const HIRAGANA = [
  // 清音（46）
  'あ',
  'い',
  'う',
  'え',
  'お',
  'か',
  'き',
  'く',
  'け',
  'こ',
  'さ',
  'し',
  'す',
  'せ',
  'そ',
  'た',
  'ち',
  'つ',
  'て',
  'と',
  'な',
  'に',
  'ぬ',
  'ね',
  'の',
  'は',
  'ひ',
  'ふ',
  'へ',
  'ほ',
  'ま',
  'み',
  'む',
  'め',
  'も',
  'や',
  'ゆ',
  'よ',
  'ら',
  'り',
  'る',
  'れ',
  'ろ',
  'わ',
  'を',
  'ん',
  // 濁音（20）
  'が',
  'ぎ',
  'ぐ',
  'げ',
  'ご',
  'ざ',
  'じ',
  'ず',
  'ぜ',
  'ぞ',
  'だ',
  'ぢ',
  'づ',
  'で',
  'ど',
  'ば',
  'び',
  'ぶ',
  'べ',
  'ぼ',
  // 半濁音（5）
  'ぱ',
  'ぴ',
  'ぷ',
  'ぺ',
  'ぽ',
  // 小書き（12）
  'ぁ',
  'ぃ',
  'ぅ',
  'ぇ',
  'ぉ',
  'っ',
  'ゃ',
  'ゅ',
  'ょ',
  'ゎ',
  'ゐ',
  'ゑ',
];

// カタカナ（87文字）
export const KATAKANA = [
  // 清音（46）
  'ア',
  'イ',
  'ウ',
  'エ',
  'オ',
  'カ',
  'キ',
  'ク',
  'ケ',
  'コ',
  'サ',
  'シ',
  'ス',
  'セ',
  'ソ',
  'タ',
  'チ',
  'ツ',
  'テ',
  'ト',
  'ナ',
  'ニ',
  'ヌ',
  'ネ',
  'ノ',
  'ハ',
  'ヒ',
  'フ',
  'ヘ',
  'ホ',
  'マ',
  'ミ',
  'ム',
  'メ',
  'モ',
  'ヤ',
  'ユ',
  'ヨ',
  'ラ',
  'リ',
  'ル',
  'レ',
  'ロ',
  'ワ',
  'ヲ',
  'ン',
  // 濁音（20）
  'ガ',
  'ギ',
  'グ',
  'ゲ',
  'ゴ',
  'ザ',
  'ジ',
  'ズ',
  'ゼ',
  'ゾ',
  'ダ',
  'ヂ',
  'ヅ',
  'デ',
  'ド',
  'バ',
  'ビ',
  'ブ',
  'ベ',
  'ボ',
  // 半濁音（5）
  'パ',
  'ピ',
  'プ',
  'ペ',
  'ポ',
  // 小書き（12）
  'ァ',
  'ィ',
  'ゥ',
  'ェ',
  'ォ',
  'ッ',
  'ャ',
  'ュ',
  'ョ',
  'ヮ',
  'ヰ',
  'ヱ',
  // その他（4）
  'ー',
  'ヴ',
  'ヵ',
  'ヶ',
];

// 英大文字（26）
export const UPPERCASE = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('');

// 英小文字（26）
export const LOWERCASE = 'abcdefghijklmnopqrstuvwxyz'.split('');

// 数字（10）
export const DIGITS = '0123456789'.split('');

// ASCII記号（33）
export const ASCII_SYMBOLS = [
  ' ',
  '!',
  '"',
  '#',
  '$',
  '%',
  '&',
  "'",
  '(',
  ')',
  '*',
  '+',
  ',',
  '-',
  '.',
  '/',
  ':',
  ';',
  '<',
  '=',
  '>',
  '?',
  '@',
  '[',
  '\\',
  ']',
  '^',
  '_',
  '`',
  '{',
  '|',
  '}',
  '~',
];

// 日本語記号（7）
export const JP_SYMBOLS = ['。', '、', '・', '「', '」', '（', '）'];

// 1ページあたりの文字数（中心マーカーが1セル占有するため COLS × ROWS - 1）
import { COLS, ROWS } from '../lib/template/layout';
export const CHARS_PER_PAGE = COLS * ROWS - 1;

// マス数/文字
export const CELLS_PER_CHAR = 2;

/**
 * 文字セット選択（ひらがな／カタカナ／英数記号／漢字）
 *
 * PDF 生成側と scanner 側で同じ選択情報を共有するために QR ペイロードに
 * `selectionToFlag()` でエンコードして載せる（v:3 フォーマット、Issue #91）。
 */
export interface CharSelection {
  hiragana: boolean;
  katakana: boolean;
  alphanum: boolean;
  kanji: boolean;
}

/**
 * 選択情報に基づいて文字リストを構築する。
 * 順序は `HIRAGANA, KATAKANA, (UPPERCASE+LOWERCASE+DIGITS+ASCII_SYMBOLS+JP_SYMBOLS), JOYO_KANJI` で固定。
 */
export function buildCharListFromSelection(sel: CharSelection): string[] {
  const chars: string[] = [];
  if (sel.hiragana) chars.push(...HIRAGANA);
  if (sel.katakana) chars.push(...KATAKANA);
  if (sel.alphanum)
    chars.push(...UPPERCASE, ...LOWERCASE, ...DIGITS, ...ASCII_SYMBOLS, ...JP_SYMBOLS);
  if (sel.kanji) chars.push(...JOYO_KANJI);
  return chars;
}

/**
 * 選択情報を QR ペイロード用のフラグ文字列に変換する。
 * 'h'=ひらがな / 'k'=カタカナ / 'a'=英数記号 / 'j'=漢字。順序は h→k→a→j 固定。
 *
 * 注意: 全 false な `CharSelection` は空文字列 `''` にエンコードされる。これは
 * `flagToSelection('')` で `null`（不正な選択）になるため、`selectionToFlag` と
 * `flagToSelection` のラウンドトリップは「全 false 状態」では意図的に復元できない。
 * UI 側で「1つも選択していない」状態は PDF 生成前にガードする前提（Template.tsx 参照）。
 */
export function selectionToFlag(sel: CharSelection): string {
  let flag = '';
  if (sel.hiragana) flag += 'h';
  if (sel.katakana) flag += 'k';
  if (sel.alphanum) flag += 'a';
  if (sel.kanji) flag += 'j';
  return flag;
}

/**
 * フラグ文字列から選択情報を復元する。
 * 不正な文字が含まれているか、空文字列の場合は null を返す。
 *
 * 注意: `selectionToFlag({...all false})` は `''` を返す仕様のため、
 * 「全 false な CharSelection」はラウンドトリップで復元できない。
 * 呼び出し側は `null` を「選択なし＝不正」として扱う（Rust 側 `parse_qr_payload`
 * も `s: ""` を同様に reject する）。
 */
export function flagToSelection(flag: string): CharSelection | null {
  if (flag.length === 0) return null;
  const sel: CharSelection = {
    hiragana: false,
    katakana: false,
    alphanum: false,
    kanji: false,
  };
  for (const ch of flag) {
    switch (ch) {
      case 'h':
        sel.hiragana = true;
        break;
      case 'k':
        sel.katakana = true;
        break;
      case 'a':
        sel.alphanum = true;
        break;
      case 'j':
        sel.kanji = true;
        break;
      default:
        return null;
    }
  }
  return sel;
}

/** 選択情報から総ページ数を算出する */
export function getTotalPages(selection: CharSelection): number {
  return Math.ceil(buildCharListFromSelection(selection).length / CHARS_PER_PAGE);
}

// ページごとの文字を取得
export function getCharactersForPage(pageIndex: number, selection: CharSelection): string[] {
  const chars = buildCharListFromSelection(selection);
  const start = pageIndex * CHARS_PER_PAGE;
  return chars.slice(start, start + CHARS_PER_PAGE);
}
