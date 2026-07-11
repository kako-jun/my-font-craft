// テンプレートレイアウト定数（mm単位）
// 1mm = 2.83465pt (72dpi / 25.4mm)
const MM_TO_PT = 72 / 25.4;

export function mm(value: number): number {
  return value * MM_TO_PT;
}

// 用紙
export const PAGE_WIDTH = 210; // A4
export const PAGE_HEIGHT = 297;
export const MARGIN = 10;

// ヘッダー
export const HEADER_HEIGHT = 7; // タイトル行のみ

// 本文領域
export const BODY_START_X = MARGIN;
export const BODY_START_Y = 28; // マーカー下端(11mm)よりも下、ヘッダー領域の下に配置

// グリッド
export const COLS = 4;
export const ROWS = 12;
export const COL_WIDTH = 47;
export const ROW_HEIGHT = 20;

// マス
export const CELL_SIZE = 15;
export const INNER_SIZE = 10;
export const CHECK_HEIGHT = 3;
export const CELL_GAP = 2;
export const SAMPLE_WIDTH = 10;

// ガイド線（#111）
// セル→em 固定変換では内枠(10mm) = em-square で、内枠下端が em Y=-120
// （ideographic embox 慣例）に写る。ベースライン（em Y=0）は内枠下端の 1.2mm 上
// = -(-120) / (1000units / 10mm)。この高さに薄いシアンの水平ガイド線を引き、
// 内枠中央に縦のセンターガイドを引く（Rust 側の正本: cli/src/layout.rs
// GUIDE_BASELINE_OFFSET_MM / EMBOX_BOTTOM_Y、関係式は vectorizer.rs のテストで固定）
export const GUIDE_BASELINE_OFFSET = 1.2;

// QRコード（本文領域下、左下付近）
// 本文最終行チェック欄下端: y=266、bottomマーカー: y=287〜295
// QR下端(267+15=282)はbottomマーカー(y=287)より上で干渉なし
export const QR_X = 20;
export const QR_Y = 267;
export const QR_SIZE = 15;

// 左右縦グレースケールバー
export const GRAY_BAR_STEPS = 10;
export const GRAY_BAR_STEP_SIZE = 5; // 各ステップ 5mm幅

// 左バー: ページ左端の余白内（マーカーより外側）
export const GRAY_BAR_LEFT_X = 2;
export const GRAY_BAR_TOP_Y = 28; // BODY_START_Y に合わせる（マーカー下端から17mm）
export const GRAY_BAR_BOTTOM_Y = 272; // 本文領域下端付近（bottomマーカーy=287よりも上）

// 右バー: ページ右端の余白内
export const GRAY_BAR_RIGHT_X = 203; // 203 + 5 = 208, ページ幅210内

// シアンサンプル
export const CYAN_SAMPLE_X = 175;
export const CYAN_SAMPLE_Y = 10;
export const CYAN_SAMPLE_SIZE = 5;

// 四隅マーカー（#33 新マーカー位置: ページ端に近い配置で外挿誤差を最小化）
export const MARKER_SIZE = 8;
export const MARKERS = {
  topLeft: { x: 3, y: 3, filled: true },
  topRight: { x: 201, y: 3, filled: false },
  // 下側マーカーは用紙端に近すぎると印刷時に見切れるため、
  // マーカー下端(y+8mm)が用紙下端(297mm)より約2mm上に来る位置にする
  bottomLeft: { x: 3, y: 286.915, filled: false },
  bottomRight: { x: 201, y: 286.915, filled: false },
} as const;

// 中心マーカー（4隅マーカー矩形の幾何学的中心に配置）
// 4隅マーカー中心の幾何学的中心 = (106, 150) に合わせる
export const CENTER_MARKER_X = 103; // 106 - SIZE/2
export const CENTER_MARKER_Y = 147; // 150 - SIZE/2
export const CENTER_MARKER_SIZE = 6;

// スキップセル（中心マーカーが占有）
export const SKIPPED_ROW = 6;
export const SKIPPED_COL = 2;

export function isSkippedCell(row: number, col: number): boolean {
  return row === SKIPPED_ROW && col === SKIPPED_COL;
}

/** グリッド上の (row, col) を文字インデックス（0〜46）に変換。スキップセルなら null */
export function gridToCharIndex(row: number, col: number): number | null {
  if (isSkippedCell(row, col)) {
    return null;
  }
  const linear = row * COLS + col;
  const skipLinear = SKIPPED_ROW * COLS + SKIPPED_COL;
  return linear < skipLinear ? linear : linear - 1;
}

// 色
export const COLOR_BLACK = { r: 0, g: 0, b: 0 };
// #87: cyan を薄めに変更 (0.8→0.9)。次回印刷時から反映
export const COLOR_CYAN = { r: 0.9, g: 1, b: 1 };
export const COLOR_WHITE = { r: 1, g: 1, b: 1 };

// 1文字セルの配置座標を計算
export function getCellPosition(row: number, col: number, cellIndex: number) {
  const x =
    BODY_START_X + col * COL_WIDTH + SAMPLE_WIDTH + CELL_GAP + cellIndex * (CELL_SIZE + CELL_GAP);
  const y = BODY_START_Y + row * ROW_HEIGHT;
  return { x, y };
}

// 見本文字の位置
export function getSamplePosition(row: number, col: number) {
  const x = BODY_START_X + col * COL_WIDTH;
  const y = BODY_START_Y + row * ROW_HEIGHT;
  return { x, y };
}
