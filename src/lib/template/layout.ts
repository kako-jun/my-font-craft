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

// QRコード（本文領域下、左下付近）
// 本文最終行チェック欄下端: y=266、bottomマーカー: y=289〜297
// QR下端(267+15=282)はbottomマーカー(y=289)より上で干渉なし
export const QR_X = 20;
export const QR_Y = 267;
export const QR_SIZE = 15;

// 左右縦グレースケールバー
export const GRAY_BAR_STEPS = 10;
export const GRAY_BAR_STEP_SIZE = 5; // 各ステップ 5mm幅

// 左バー: ページ左端の余白内（マーカーより外側）
export const GRAY_BAR_LEFT_X = 2;
export const GRAY_BAR_TOP_Y = 22; // topマーカー下端(11mm)から余白を空けて開始
export const GRAY_BAR_BOTTOM_Y = 272; // 本文領域下端付近（bottomマーカーy=289よりも上）

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
  bottomLeft: { x: 3, y: 289, filled: false },
  bottomRight: { x: 201, y: 289, filled: false },
} as const;

// 色
export const COLOR_BLACK = { r: 0, g: 0, b: 0 };
export const COLOR_CYAN = { r: 0.8, g: 1, b: 1 };
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
