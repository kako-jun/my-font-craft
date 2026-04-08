// ヘッダー雑学コメント（漫画の作者コメント風）
// 日本語で短く（20-40文字程度）

// ページ番号にまつわる雑学（特定ページに表示）
const PAGE_TRIVIA: Record<number, string> = {
  1: '記念すべき1ページ目。楽しんで書こう',
  3: '三は日本で最も縁起のいい数字のひとつ',
  4: '四つ葉のクローバーは幸運の象徴',
  7: 'ラッキーセブン。いい字が書けそう',
  8: '八は末広がりで縁起がいい',
  9: '九は中国語で「久」と同じ発音',
  13: '13は西洋では不吉、でも書は関係ない',
  21: 'ブラックジャック！ 最強の手',
  33: 'ゾロ目。ちょっと嬉しい',
  42: '42は「生命、宇宙、そして万物の答え」',
  55: 'ゴーゴー！',
  64: '64は2の6乗。コンピュータ的に美しい数',
  77: 'ラッキーセブンが2つ。超ラッキー',
};

// ひらがなにまつわる雑学
const HIRAGANA_TRIVIA = [
  'ひらがなは平安時代に漢字から生まれた',
  'いろは歌には全てのひらがなが入っている',
  '「を」は昔「wo」と発音していた',
  '「ゐ」「ゑ」は現代ではほぼ使われない',
  'ひらがなは女手とも呼ばれていた',
  '清少納言もひらがなで枕草子を書いた',
  '万葉仮名がひらがなの原型',
  '「ん」はひらがなで最後に成立した文字',
  '濁点は室町時代頃から使われ始めた',
  '半濁音「ぱ行」はポルトガル語の影響説も',
];

// カタカナにまつわる雑学
const KATAKANA_TRIVIA = [
  'カタカナは漢字の一部から生まれた',
  'カタカナは僧侶の読み仮名が起源',
  '外来語をカタカナで書く習慣は明治から',
  'アイヌ語の表記にもカタカナが使われる',
  '「ー」（長音符）は意外と歴史が浅い',
  'カタカナの「ン」もひらがな同様に後発',
  '「ヴ」は明治時代に作られた比較的新しい文字',
  'カタカナは角張った形が特徴的',
];

// アルファベット・数字にまつわる雑学
const ALPHANUM_TRIVIA = [
  'Aはフェニキア文字の牛の頭が起源',
  'アルファベットは約3000年の歴史を持つ',
  '0の概念はインドで生まれた',
  'Zはギリシャ文字のゼータから',
  '英語で最も使われる文字はE',
  'Qの後にはほぼ必ずUが来る',
  'Xは未知数を表す。デカルトが始めた',
  '!は15世紀頃の写本から使われ始めた',
  '@は中世の修道士が「ad」を略した説がある',
  '&はラテン語の「et」を合体させた文字',
  '#はラテン語「libra pondo」の略号lb変形',
  'ASCIIは1963年に初版が策定された',
];

// 漢字にまつわる雑学
const KANJI_TRIVIA = [
  '常用漢字は2136字。全部書いたらすごい',
  '漢字は約3300年前の甲骨文字が起源',
  '日本の漢字には音読みと訓読みがある',
  '「鬱」は29画。常用漢字で最多画数',
  '「一」は最もシンプルな常用漢字',
  '部首は約200種類ある',
  '漢字の「字」は屋根の下で子を育てる意味',
  '「薔薇」は書けなくても読める漢字の代表',
  '「凸凹」は形がそのまま意味を表す面白い字',
  '新字体は1946年の当用漢字表で導入された',
  '漢字検定1級の合格率は約10%',
  '中国の簡体字と日本の新字体は別の簡略化',
  '「森」は木が3つ。漢字は組み合わせが面白い',
  '「品」は口が3つ。多さを表す漢字の典型',
  '国字（和製漢字）は「峠」「込」「畑」など',
  '「々」は漢字ではなく踊り字という記号',
  '漢字の総数は5万字以上あるとされる',
  '六書（象形・指事・会意・形声・転注・仮借）',
  '形声文字が漢字全体の80%以上を占める',
  '「東」は木に太陽がかかる象形が起源説',
];

// 汎用雑学（どのページでも使える）
const GENERAL_TRIVIA = [
  '手書きの文字には個性が宿る',
  '書くことは脳のトレーニングにもなる',
  '世界には約7000の言語がある',
  '日本語は3種類の文字体系を使う稀有な言語',
  'フォントの語源はラテン語の「融かす」',
  'タイポグラフィの歴史は活版印刷から',
  '明朝体は中国の明王朝にちなんだ名前',
  'ゴシック体は日本独自の呼び方',
  '手書き文字の認識精度は年々向上している',
  'カリグラフィは「美しい書法」という意味',
  '筆圧が一定だと読みやすい字になる',
  '1日10分の練習で字はきれいになる',
  'ペンの持ち方で字の印象が変わる',
  '太い線と細い線の差がフォントの味になる',
  '自分だけのフォント、完成が楽しみ',
  '丁寧に書いた字は後で見返しても嬉しい',
  '文字のバランスは中心線を意識すると安定する',
  'はみ出しても大丈夫。味になる',
  '一画一画を大切に。急がなくていい',
  '休憩も大事。疲れたら手を休めよう',
];

// シード付きシャッフル（同じシードなら同じ順序を保証）
function seededShuffle<T>(arr: T[], seed: number): T[] {
  const result = [...arr];
  let s = seed;
  for (let i = result.length - 1; i > 0; i--) {
    s = (s * 1664525 + 1013904223) & 0xffffffff;
    const j = (s >>> 0) % (i + 1);
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result;
}

// カテゴリ配列をシャッフルしてから順に取得（全要素を使い切ってから2周目）
function pickFromCategory(arr: string[], index: number, seed: number): string {
  const shuffled = seededShuffle(arr, seed);
  return shuffled[index % shuffled.length];
}

/**
 * ページに表示する雑学コメントを取得する
 * @param pageIndex 0始まりのページインデックス
 * @param pageChars そのページに含まれる文字の配列
 * @param totalPages 総ページ数
 */
export function getTriviaForPage(
  pageIndex: number,
  pageChars: string[],
  totalPages: number,
): string {
  const pageNum = pageIndex + 1;

  // 最終ページ
  if (pageNum === totalPages) {
    return '最終ページ。完走おめでとう！';
  }

  // ラスト1ページ
  if (pageNum === totalPages - 1) {
    return 'ラスト1ページ。お疲れ様！';
  }

  // 進捗マイルストーン（totalPages ベースで動的に計算）
  if (totalPages >= 4) {
    const quarter = Math.round(totalPages / 4);
    const half = Math.round(totalPages / 2);
    const threeQuarter = Math.round((totalPages * 3) / 4);
    if (pageNum === quarter) return `${pageNum}ページ。4分の1を通過！`;
    if (pageNum === half) return `${pageNum}ページ。折り返し地点！`;
    if (pageNum === threeQuarter) return `${pageNum}ページ。ゴールが見えてきた`;
  }

  // 10ページ刻みのキリ番
  if (pageNum >= 10 && pageNum % 10 === 0) {
    return `${pageNum}ページ到達。いいペース！`;
  }

  // ページ番号に固有の雑学（数字にまつわるトリビア）
  if (PAGE_TRIVIA[pageNum]) {
    return PAGE_TRIVIA[pageNum];
  }

  // totalPages をシードに使い、同じ構成なら同じ順序を保証
  const seed = totalPages * 31;

  // ページの文字種を判定して対応する雑学から選ぶ
  const firstChar = pageChars[0];
  if (firstChar) {
    const code = firstChar.charCodeAt(0);
    // ひらがな: U+3040-U+309F
    if (code >= 0x3040 && code <= 0x309f) {
      return pickFromCategory(HIRAGANA_TRIVIA, pageIndex, seed);
    }
    // カタカナ: U+30A0-U+30FF
    if (code >= 0x30a0 && code <= 0x30ff) {
      return pickFromCategory(KATAKANA_TRIVIA, pageIndex, seed + 1);
    }
    // ASCII
    if (code < 0x80) {
      return pickFromCategory(ALPHANUM_TRIVIA, pageIndex, seed + 2);
    }
    // 漢字（CJK統合漢字）
    if (code >= 0x4e00 && code <= 0x9fff) {
      return pickFromCategory(KANJI_TRIVIA, pageIndex, seed + 3);
    }
  }

  // フォールバック: 汎用雑学
  return pickFromCategory(GENERAL_TRIVIA, pageIndex, seed + 4);
}
