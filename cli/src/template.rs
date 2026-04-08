/// テスト画像生成（generate サブコマンド）
/// A4 300dpi のテンプレートPNG相当を直接描画する
use image::{RgbaImage, Rgba};
use crate::layout;
use crate::qr;
use std::path::Path;

/// テンプレート画像を生成して保存
pub fn generate_template(output_path: &Path, with_glyphs: bool) -> Result<(), String> {
    let w = layout::image_width();
    let h = layout::image_height();
    println!("テンプレート生成: {w}x{h} px (A4 300dpi)");

    // 白背景
    let mut img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    // 四隅マーカー
    draw_markers(&mut img);

    // セルグリッド
    draw_cell_grid(&mut img);

    // 中心マーカー（グリッド後に描画して罫線を上書き+白境界で隔離）
    draw_center_marker(&mut img);

    // グレースケールバー
    draw_gray_bars(&mut img);

    // QRコード
    let qr_data = serde_json::json!({
        "p": "mfc",
        "v": 2,
        "pg": 1,
        "t": 1,
        "m": 2
    });
    qr::draw_qr_on_image(&mut img, &qr_data.to_string())?;
    println!("  QRコード描画完了");

    // シアンサンプル
    draw_cyan_sample(&mut img);

    // ダミー文字（--with-glyphs 時）
    if with_glyphs {
        draw_dummy_glyphs(&mut img);
    }

    // 保存
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;
    }
    img.save(output_path)
        .map_err(|e| format!("画像保存エラー: {e}"))?;
    println!("テンプレート保存: {}", output_path.display());

    Ok(())
}

/// 四隅マーカーを描画
fn draw_markers(img: &mut RgbaImage) {
    let markers = [
        layout::MARKER_TL,
        layout::MARKER_TR,
        layout::MARKER_BL,
        layout::MARKER_BR,
    ];

    for m in &markers {
        let (cx, cy) = layout::marker_center(m);
        let cx_px = layout::mm_to_px(cx).round() as i32;
        let cy_px = layout::mm_to_px(cy).round() as i32;
        let r = (layout::mm_to_px(layout::MARKER_SIZE / 2.0)).round() as i32;

        if m.filled {
            // 塗りつぶし円
            draw_filled_circle(img, cx_px, cy_px, r, Rgba([0, 0, 0, 255]));
        } else {
            // 枠線のみの円（線幅2px程度）
            draw_circle_outline(img, cx_px, cy_px, r, 2, Rgba([0, 0, 0, 255]));
        }
    }
    println!("  マーカー描画完了 (TL=filled, TR/BL/BR=outline)");
}

/// 塗りつぶし円を描画
fn draw_filled_circle(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, color: Rgba<u8>) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

/// 枠線のみの円を描画
fn draw_circle_outline(img: &mut RgbaImage, cx: i32, cy: i32, radius: i32, thickness: i32, color: Rgba<u8>) {
    let r_outer = radius;
    let r_inner = (radius - thickness).max(0);
    for dy in -r_outer..=r_outer {
        for dx in -r_outer..=r_outer {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= r_outer * r_outer && dist_sq >= r_inner * r_inner {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, color);
                }
            }
        }
    }
}

/// 中心マーカーを描画（塗りつぶし四角 + 白アイソレーション境界）
/// グリッド罫線との接触を防ぐため、マーカー周囲に白い境界を確保してから描画
fn draw_center_marker(img: &mut RgbaImage) {
    let x = layout::mm_to_px(layout::CENTER_MARKER_X).round() as u32;
    let y = layout::mm_to_px(layout::CENTER_MARKER_Y).round() as u32;
    let size = layout::mm_to_px(layout::CENTER_MARKER_SIZE).round() as u32;
    let border = 4u32; // 白アイソレーション境界（px）
    let white = Rgba([255, 255, 255, 255]);
    let black = Rgba([0, 0, 0, 255]);
    // 周囲の罫線を白で上書きして隔離
    fill_rect(img, x.saturating_sub(border), y.saturating_sub(border),
              size + border * 2, size + border * 2, white);
    // 塗りつぶし四角
    fill_rect(img, x, y, size, size, black);
    println!("  中心マーカー描画完了 ({}x{}mm 塗りつぶし四角、白境界付き)", layout::CENTER_MARKER_SIZE, layout::CENTER_MARKER_SIZE);
}

/// セルグリッドを描画
fn draw_cell_grid(img: &mut RgbaImage) {
    let black = Rgba([0, 0, 0, 255]);
    let cyan = Rgba([layout::COLOR_CYAN[0], layout::COLOR_CYAN[1], layout::COLOR_CYAN[2], 255]);

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);
                let px_x = layout::mm_to_px(mm_x).round() as u32;
                let px_y = layout::mm_to_px(mm_y).round() as u32;
                let cell_px = layout::mm_to_px(layout::CELL_SIZE).round() as u32;
                let inner_px = layout::mm_to_px(layout::INNER_SIZE).round() as u32;
                let check_px = layout::mm_to_px(layout::CHECK_HEIGHT).round() as u32;

                // 黒の外枠（15mm四方）
                draw_rect_outline(img, px_x, px_y, cell_px, cell_px, black);

                // シアンの内枠（10mm四方、中央配置）
                let offset = (cell_px - inner_px) / 2;
                draw_rect_outline(img, px_x + offset, px_y + offset, inner_px, inner_px, cyan);

                // チェック欄（3mm、セル下部）
                let check_y = px_y + cell_px - check_px;
                draw_rect_outline(img, px_x, check_y, cell_px, check_px, black);
            }

            // 見本文字エリア（10mm四方）
            let (sx, sy) = layout::get_sample_position(row, col);
            let spx = layout::mm_to_px(sx).round() as u32;
            let spy = layout::mm_to_px(sy).round() as u32;
            let sample_px = layout::mm_to_px(layout::SAMPLE_WIDTH).round() as u32;
            let cell_h_px = layout::mm_to_px(layout::CELL_SIZE).round() as u32;
            draw_rect_outline(img, spx, spy, sample_px, cell_h_px, black);
        }
    }
    println!("  セルグリッド描画完了 ({}列×{}行×2マス)", layout::COLS, layout::ROWS);
}

/// 矩形の外枠を描画
fn draw_rect_outline(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    let max_x = (x + w).min(img.width());
    let max_y = (y + h).min(img.height());

    // 上辺・下辺
    for px in x..max_x {
        if y < img.height() {
            img.put_pixel(px, y, color);
        }
        if max_y > 0 && max_y - 1 < img.height() {
            img.put_pixel(px, max_y - 1, color);
        }
    }
    // 左辺・右辺
    for py in y..max_y {
        if x < img.width() {
            img.put_pixel(x, py, color);
        }
        if max_x > 0 && max_x - 1 < img.width() {
            img.put_pixel(max_x - 1, py, color);
        }
    }
}

/// グレースケールバーを描画（左右に10段階）
fn draw_gray_bars(img: &mut RgbaImage) {
    let bar_w_px = layout::mm_to_px(layout::GRAY_BAR_STEP_SIZE).round() as u32;

    // 左右バーのY座標範囲
    let top_y = layout::mm_to_px(layout::GRAY_BAR_TOP_Y).round() as u32;
    let bottom_y = layout::mm_to_px(layout::GRAY_BAR_BOTTOM_Y).round() as u32;
    let total_h = bottom_y - top_y;

    // 10段階を均等配置
    let actual_step_h = total_h / layout::GRAY_BAR_STEPS as u32;

    for i in 0..layout::GRAY_BAR_STEPS {
        let intensity = (i as f64 / layout::GRAY_BAR_STEPS as f64 * 255.0).round() as u8;
        let color = Rgba([intensity, intensity, intensity, 255]);
        let y_start = top_y + i as u32 * actual_step_h;

        // 左バー
        let left_x = layout::mm_to_px(layout::GRAY_BAR_LEFT_X).round() as u32;
        fill_rect(img, left_x, y_start, bar_w_px, actual_step_h, color);

        // 右バー
        let right_x = layout::mm_to_px(layout::GRAY_BAR_RIGHT_X).round() as u32;
        fill_rect(img, right_x, y_start, bar_w_px, actual_step_h, color);
    }
    println!("  グレースケールバー描画完了 ({}段階)", layout::GRAY_BAR_STEPS);
}

/// 塗りつぶし矩形
fn fill_rect(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, color: Rgba<u8>) {
    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                img.put_pixel(px, py, color);
            }
        }
    }
}

/// ダミー文字とチェックマークを描画（採用判定テスト用）
/// 全パターンを網羅的にテスト
fn draw_dummy_glyphs(img: &mut RgbaImage) {
    let black = Rgba([0, 0, 0, 255]);
    let inner_offset = (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0;

    // (row, col, cell_idx, fill_ratio, check_mark)
    // check_mark: None=空欄, Some(true)=✓, Some(false)=×
    let test_cells: &[(usize, usize, usize, f64, Option<bool>)] = &[
        // Row0 Col0: 両方記入、マークなし → 右(I1)を採用
        (0, 0, 0, 0.5, None),
        (0, 0, 1, 0.5, None),
        // Row0 Col1: 左に✓ → 左(I0)を採用
        (0, 1, 0, 0.5, Some(true)),
        (0, 1, 1, 0.5, None),
        // Row0 Col2: 両方✓ → 両方バリエーション採用
        (0, 2, 0, 0.5, Some(true)),
        (0, 2, 1, 0.5, Some(true)),
        // Row0 Col3: 右に× → 左(I0)を採用
        (0, 3, 0, 0.5, None),
        (0, 3, 1, 0.5, Some(false)),
        // Row1 Col0: 片方だけ記入 → それを採用
        (1, 0, 0, 0.5, None),
        // Row1 Col1: 左に×、右に記入 → 右(I1)を採用
        (1, 1, 0, 0.5, Some(false)),
        (1, 1, 1, 0.5, None),
        // Row1 Col2: 両方空 → 採用なし（何も描画しない）
        // Row1 Col3: 小さい文字の検出限界テスト
        (1, 3, 0, 0.1, None),
    ];

    for &(row, col, cell_idx, fill_ratio, check_mark) in test_cells {
        // 文字領域にダミー描画
        let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);
        let inner_px = layout::mm_to_px(layout::INNER_SIZE).round() as u32;
        let px_x = layout::mm_to_px(mm_x + inner_offset).round() as u32;
        let px_y = layout::mm_to_px(mm_y + inner_offset).round() as u32;
        let glyph_size = (inner_px as f64 * fill_ratio).round() as u32;
        let glyph_offset = (inner_px - glyph_size) / 2;
        fill_rect(img, px_x + glyph_offset, px_y + glyph_offset, glyph_size, glyph_size, black);

        // チェック欄にマーク描画
        if let Some(is_check) = check_mark {
            let cell_px = layout::mm_to_px(layout::CELL_SIZE).round() as u32;
            let check_h = layout::mm_to_px(layout::CHECK_HEIGHT).round() as u32;
            let check_x = layout::mm_to_px(mm_x).round() as u32;
            let check_y = layout::mm_to_px(mm_y + layout::CELL_SIZE).round() as u32;

            if is_check {
                // ✓マーク: チェック欄の中央に太いV字
                let mid_x = check_x + cell_px / 2;
                let mid_y = check_y + check_h / 2;
                let arm = (check_h as i32).max(8);
                // 左下がり短線
                for i in 0..arm {
                    for t in -2i32..=2 {
                        let px = (mid_x as i32 - arm / 2 + i) as u32;
                        let py = (mid_y as i32 - arm / 4 + i / 2 + t) as u32;
                        if px < img.width() && py < img.height() {
                            img.put_pixel(px, py, black);
                        }
                    }
                }
                // 右上がり長線
                for i in 0..(arm * 2) {
                    for t in -2i32..=2 {
                        let px = (mid_x as i32 - arm / 2 + arm + i) as u32;
                        let py = (mid_y as i32 + arm / 4 - i / 3 + t) as u32;
                        if px < img.width() && py > 0 && py < img.height() {
                            img.put_pixel(px, py, black);
                        }
                    }
                }
            } else {
                // ×マーク: 太い対角線2本
                let margin = 4u32;
                let w = cell_px - margin * 2;
                let h = check_h - margin;
                for i in 0..w {
                    let x1 = check_x + margin + i;
                    let y1 = check_y + margin / 2 + (i * h / w);
                    let y2 = check_y + check_h - margin / 2 - (i * h / w);
                    for dy in 0..3 {
                        if x1 < img.width() {
                            if y1 + dy < img.height() { img.put_pixel(x1, y1 + dy, black); }
                            if y2 + dy < img.height() { img.put_pixel(x1, y2 + dy, black); }
                        }
                    }
                }
            }
        }
    }

    println!("  ダミー文字+チェックマーク描画完了 (採用判定テスト用)");
}

/// シアンサンプルを描画
fn draw_cyan_sample(img: &mut RgbaImage) {
    let x = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
    let y = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
    let size = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;
    let cyan = Rgba([layout::COLOR_CYAN[0], layout::COLOR_CYAN[1], layout::COLOR_CYAN[2], 255]);
    fill_rect(img, x, y, size, size, cyan);
    println!("  シアンサンプル描画完了");
}
