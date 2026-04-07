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

// 全文字リスト
export const ALL_CHARACTERS = [
  ...HIRAGANA,
  ...KATAKANA,
  ...UPPERCASE,
  ...LOWERCASE,
  ...DIGITS,
  ...ASCII_SYMBOLS,
  ...JP_SYMBOLS,
  ...JOYO_KANJI,
];

// 1ページあたりの文字数（layout.ts の COLS × ROWS から導出）
import { COLS, ROWS } from '../lib/template/layout';
export const CHARS_PER_PAGE = COLS * ROWS;

// マス数/文字
export const CELLS_PER_CHAR = 2;

// 総ページ数
export const TOTAL_PAGES = Math.ceil(ALL_CHARACTERS.length / CHARS_PER_PAGE);

// ページごとの文字を取得
export function getCharactersForPage(pageIndex: number): string[] {
  const start = pageIndex * CHARS_PER_PAGE;
  return ALL_CHARACTERS.slice(start, start + CHARS_PER_PAGE);
}
