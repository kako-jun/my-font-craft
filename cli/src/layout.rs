// テンプレートレイアウト定数（mm単位）
// TypeScript layout.ts からの移植 + #33 新マーカー位置

// DPI
pub const DPI: f64 = 300.0;

/// mm → px 変換（300dpi）
pub fn mm_to_px(mm: f64) -> f64 {
    mm * DPI / 25.4
}

// 用紙（mm）
pub const PAGE_WIDTH: f64 = 210.0;
pub const PAGE_HEIGHT: f64 = 297.0;
pub const MARGIN: f64 = 10.0;

// ヘッダー
pub const HEADER_HEIGHT: f64 = 7.0;

// 本文領域
pub const BODY_START_X: f64 = 10.0; // = MARGIN
pub const BODY_START_Y: f64 = 28.0;

// グリッド
pub const COLS: usize = 4;
pub const ROWS: usize = 12;
pub const CHARS_PER_PAGE: usize = 48; // COLS * ROWS（文字数。各文字に2マスあるのでセル総数は96）
pub const COL_WIDTH: f64 = 47.0;
pub const ROW_HEIGHT: f64 = 20.0;

// マス
pub const CELL_SIZE: f64 = 15.0;
pub const INNER_SIZE: f64 = 10.0;
pub const CHECK_HEIGHT: f64 = 3.0;
pub const CELL_GAP: f64 = 2.0;
pub const SAMPLE_WIDTH: f64 = 10.0;

// QRコード
pub const QR_X: f64 = 20.0;
pub const QR_Y: f64 = 267.0;
pub const QR_SIZE: f64 = 15.0;

// グレースケールバー
pub const GRAY_BAR_STEPS: usize = 10;
pub const GRAY_BAR_STEP_SIZE: f64 = 5.0;
pub const GRAY_BAR_LEFT_X: f64 = 2.0;
pub const GRAY_BAR_RIGHT_X: f64 = 203.0;
pub const GRAY_BAR_TOP_Y: f64 = 17.0;
pub const GRAY_BAR_BOTTOM_Y: f64 = 272.0;

// シアンサンプル
pub const CYAN_SAMPLE_X: f64 = 175.0;
pub const CYAN_SAMPLE_Y: f64 = 10.0;
pub const CYAN_SAMPLE_SIZE: f64 = 5.0;

// 四隅マーカー（#33 新マーカー位置）
pub const MARKER_SIZE: f64 = 8.0;

#[derive(Debug, Clone, Copy)]
pub struct MarkerDef {
    pub x: f64,
    pub y: f64,
    pub filled: bool,
}

pub const MARKER_TL: MarkerDef = MarkerDef { x: 3.0, y: 3.0, filled: true };
pub const MARKER_TR: MarkerDef = MarkerDef { x: 201.0, y: 3.0, filled: false };
pub const MARKER_BL: MarkerDef = MarkerDef { x: 3.0, y: 289.0, filled: false };
pub const MARKER_BR: MarkerDef = MarkerDef { x: 201.0, y: 289.0, filled: false };

/// マーカー中心座標（mm）
pub fn marker_center(m: &MarkerDef) -> (f64, f64) {
    (m.x + MARKER_SIZE / 2.0, m.y + MARKER_SIZE / 2.0)
}

// 色
pub const COLOR_CYAN: [u8; 3] = [204, 255, 255]; // 0.8*255, 1*255, 1*255

// 画像サイズ（300dpi）
pub fn image_width() -> u32 {
    mm_to_px(PAGE_WIDTH).round() as u32
}

pub fn image_height() -> u32 {
    mm_to_px(PAGE_HEIGHT).round() as u32
}

/// 1文字セルの配置座標を計算（mm）
pub fn get_cell_position(row: usize, col: usize, cell_index: usize) -> (f64, f64) {
    let x = BODY_START_X
        + col as f64 * COL_WIDTH
        + SAMPLE_WIDTH
        + CELL_GAP
        + cell_index as f64 * (CELL_SIZE + CELL_GAP);
    let y = BODY_START_Y + row as f64 * ROW_HEIGHT;
    (x, y)
}

/// 見本文字の位置（mm）
pub fn get_sample_position(row: usize, col: usize) -> (f64, f64) {
    let x = BODY_START_X + col as f64 * COL_WIDTH;
    let y = BODY_START_Y + row as f64 * ROW_HEIGHT;
    (x, y)
}
