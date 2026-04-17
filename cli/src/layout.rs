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
/// 中心マーカーが1セル分を占有するため 4×12 - 1 = 47
pub const CHARS_PER_PAGE: usize = COLS * ROWS - 1;
/// 中心マーカーに占有されるセル位置
pub const SKIPPED_ROW: usize = 6;
pub const SKIPPED_COL: usize = 2;
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

// グレースケールバー（マーカー quiet zone を避けて配置）
// 最も黒い step(0) がマーカー探索領域(25%)の端に来るよう、本文開始位置から描画
pub const GRAY_BAR_STEPS: usize = 10;
pub const GRAY_BAR_STEP_SIZE: f64 = 5.0;
pub const GRAY_BAR_LEFT_X: f64 = 2.0;
pub const GRAY_BAR_RIGHT_X: f64 = 203.0;
pub const GRAY_BAR_TOP_Y: f64 = 28.0;    // BODY_START_Y に合わせる（マーカー下端から17mm）
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
// 下側マーカーは用紙端に近すぎると印刷時に見切れるため、
// マーカー下端(y+8mm)が用紙下端(297mm)より約2mm上に来る位置にする
pub const MARKER_BL: MarkerDef = MarkerDef { x: 3.0, y: 286.915, filled: false };
pub const MARKER_BR: MarkerDef = MarkerDef { x: 201.0, y: 286.915, filled: false };

/// 中心マーカー（検証・レンズ歪み検出用、塗りつぶし四角）
/// 4隅マーカー中心の幾何学的中心 = (106, 150) に合わせる
pub const CENTER_MARKER_X: f64 = 103.0; // 106 - SIZE/2
pub const CENTER_MARKER_Y: f64 = 147.0; // 150 - SIZE/2
pub const CENTER_MARKER_SIZE: f64 = 6.0;

/// 四隅マーカー中心座標（mm）
pub fn marker_center(m: &MarkerDef) -> (f64, f64) {
    (m.x + MARKER_SIZE / 2.0, m.y + MARKER_SIZE / 2.0)
}

/// 中心マーカー中心座標（mm）
pub fn center_marker_center() -> (f64, f64) {
    (CENTER_MARKER_X + CENTER_MARKER_SIZE / 2.0, CENTER_MARKER_Y + CENTER_MARKER_SIZE / 2.0)
}

// 色
// #87: cyan を薄めに変更 (204→230)。次回印刷時から反映
pub const COLOR_CYAN: [u8; 3] = [230, 255, 255]; // 0.9*255, 1*255, 1*255

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

/// 中心マーカーに占有されたセルかどうかを判定
pub fn is_skipped_cell(row: usize, col: usize) -> bool {
    row == SKIPPED_ROW && col == SKIPPED_COL
}

/// グリッド上の (row, col) を文字インデックス（0〜46）に変換
/// スキップセルの場合は None を返す
pub fn grid_to_char_index(row: usize, col: usize) -> Option<usize> {
    if is_skipped_cell(row, col) {
        return None;
    }
    let linear = row * COLS + col;
    let skip_linear = SKIPPED_ROW * COLS + SKIPPED_COL;
    if linear < skip_linear {
        Some(linear)
    } else {
        Some(linear - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TS版の計算式:
    ///   x = 10 + col*47 + 10 + 2 + cell_index*(15+2)
    ///   y = 28 + row*20
    fn ts_get_cell_position(row: usize, col: usize, cell_index: usize) -> (f64, f64) {
        let x = 10.0 + col as f64 * 47.0 + 10.0 + 2.0 + cell_index as f64 * (15.0 + 2.0);
        let y = 28.0 + row as f64 * 20.0;
        (x, y)
    }

    #[test]
    fn cell_position_row0_col0_i0() {
        let (x, y) = get_cell_position(0, 0, 0);
        let (ex, ey) = ts_get_cell_position(0, 0, 0);
        assert!((x - ex).abs() < 1e-9, "x: got={x}, expected={ex}");
        assert!((y - ey).abs() < 1e-9, "y: got={y}, expected={ey}");
    }

    #[test]
    fn cell_position_row0_col0_i1() {
        let (x, y) = get_cell_position(0, 0, 1);
        let (ex, ey) = ts_get_cell_position(0, 0, 1);
        assert!((x - ex).abs() < 1e-9, "x: got={x}, expected={ex}");
        assert!((y - ey).abs() < 1e-9, "y: got={y}, expected={ey}");
    }

    #[test]
    fn cell_position_row3_col2_i0() {
        let (x, y) = get_cell_position(3, 2, 0);
        let (ex, ey) = ts_get_cell_position(3, 2, 0);
        assert!((x - ex).abs() < 1e-9, "x: got={x}, expected={ex}");
        assert!((y - ey).abs() < 1e-9, "y: got={y}, expected={ey}");
    }

    #[test]
    fn cell_position_row11_col3_i1() {
        let (x, y) = get_cell_position(11, 3, 1);
        let (ex, ey) = ts_get_cell_position(11, 3, 1);
        assert!((x - ex).abs() < 1e-9, "x: got={x}, expected={ex}");
        assert!((y - ey).abs() < 1e-9, "y: got={y}, expected={ey}");
    }

    #[test]
    fn cell_position_all_chars() {
        // 全セル×2マスがTS版と一致することを確認（スキップセル含む）
        for row in 0..ROWS {
            for col in 0..COLS {
                for ci in 0..2 {
                    let (x, y) = get_cell_position(row, col, ci);
                    let (ex, ey) = ts_get_cell_position(row, col, ci);
                    assert!(
                        (x - ex).abs() < 1e-9 && (y - ey).abs() < 1e-9,
                        "mismatch at row={row}, col={col}, ci={ci}: ({x},{y}) vs ({ex},{ey})"
                    );
                }
            }
        }
    }

    #[test]
    fn chars_per_page_is_47() {
        assert_eq!(CHARS_PER_PAGE, 47);
    }

    #[test]
    fn skipped_cell_is_excluded() {
        assert!(is_skipped_cell(SKIPPED_ROW, SKIPPED_COL));
        assert!(!is_skipped_cell(0, 0));
        assert!(!is_skipped_cell(SKIPPED_ROW, 0));
    }

    #[test]
    fn grid_to_char_index_covers_47() {
        let mut count = 0;
        for row in 0..ROWS {
            for col in 0..COLS {
                if let Some(idx) = grid_to_char_index(row, col) {
                    assert!(idx < CHARS_PER_PAGE, "idx={idx} out of range");
                    count += 1;
                }
            }
        }
        assert_eq!(count, CHARS_PER_PAGE);
    }

    #[test]
    fn mm_to_px_known_values() {
        // 25.4mm = 1inch = 300px at 300dpi
        assert!((mm_to_px(25.4) - 300.0).abs() < 1e-9);
        assert!((mm_to_px(0.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn skipped_cell_correct() {
        assert!(is_skipped_cell(6, 2));
        assert!(!is_skipped_cell(5, 2));
        assert!(!is_skipped_cell(6, 1));
        assert!(!is_skipped_cell(0, 0));
        assert!(!is_skipped_cell(11, 3));
    }
}
