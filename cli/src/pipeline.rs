/// 画像処理パイプライン（process サブコマンド）
use image::{DynamicImage, GrayImage, RgbaImage, Rgba};
use std::path::Path;
use crate::{layout, marker, perspective, qr, cell};
// perspective::sample_bilinear を直交性補正で使用

/// パイプラインを実行
pub fn run_pipeline(image_path: &Path, output_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;

    // ステップ1: 画像読み込み
    println!("\n=== ステップ1: 画像読み込み ===");
    let img = image::open(image_path)
        .map_err(|e| format!("画像読み込みエラー: {e}"))?;
    let rgba = img.to_rgba8();
    println!("  画像サイズ: {}x{}", rgba.width(), rgba.height());
    rgba.save(output_dir.join("01_input.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 01_input.png 保存完了");

    // ステップ2: 二値化
    println!("\n=== ステップ2: 二値化 ===");
    let gray = DynamicImage::ImageRgba8(rgba.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    println!("  大津の閾値: {threshold}");
    let binary = marker::binarize(&gray, threshold);
    binary
        .save(output_dir.join("02_binary.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 02_binary.png 保存完了");

    // ステップ3: マーカー検出
    println!("\n=== ステップ3: マーカー検出 ===");
    let markers = marker::detect_markers(&binary)?;
    let marker_img = marker::draw_marker_overlay(&rgba, &markers);
    marker_img
        .save(output_dir.join("03_markers.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 03_markers.png 保存完了");

    // ステップ4: 向き検出
    println!("\n=== ステップ4: 向き検出 ===");
    let (tl_index, rotation) = marker::detect_orientation(&binary, &markers)?;

    let (oriented_img, oriented_markers) = if rotation != 0 {
        println!("  画像を{rotation}°回転します");
        let rotated = marker::rotate_image(&rgba, rotation);
        let reordered = marker::reorder_markers(&markers, tl_index, rotation, rgba.width(), rgba.height());
        (rotated, reordered)
    } else {
        (rgba.clone(), markers.clone())
    };

    oriented_img
        .save(output_dir.join("04_oriented.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 04_oriented.png 保存完了");

    // ステップ5+6: マーカー4点から直接ホモグラフィー変換（外挿廃止）
    println!("\n=== ステップ5+6: マーカー直接ホモグラフィー変換 ===");
    let corrected = perspective::homography_warp_from_markers(&oriented_img, &oriented_markers);
    corrected
        .save(output_dir.join("05_corrected.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 05_corrected.png 保存完了");

    // ステップ6.5: 補正品質チェック（マーカー再検出+残差計測）
    println!("\n=== ステップ6.5: 補正品質チェック ===");
    verify_correction_quality(&corrected, output_dir);

    // ステップ6.6: 中心マーカー検証
    println!("\n=== ステップ6.6: 中心マーカー検証 ===");
    verify_center_marker(&corrected);

    // ステップ6.7: 罫線直交性チェック＋微小回転補正
    println!("\n=== ステップ6.7: 罫線直交性チェック ===");
    let corrected = apply_orthogonality_correction(corrected, output_dir);

    // ステップ7: QR読み取り
    println!("\n=== ステップ7: QR読み取り ===");
    let qr_result = read_qr_from_corrected(&corrected, output_dir);
    match &qr_result {
        Ok(data) => println!("  QRデータ: {data}"),
        Err(e) => println!("  QR読み取り失敗（続行）: {e}"),
    }

    // ステップ8: 影補正
    println!("\n=== ステップ8: 影補正 ===");
    let shadow_corrected = correct_shadow(&corrected);
    shadow_corrected
        .save(output_dir.join("07_shadow_corrected.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 07_shadow_corrected.png 保存完了");

    // ステップ9: シアン除去
    println!("\n=== ステップ9: シアン除去 ===");
    let cyan_removed = remove_cyan(&shadow_corrected);
    cyan_removed
        .save(output_dir.join("08_cyan_removed.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 08_cyan_removed.png 保存完了");

    // ステップ10: セル切り出し + チェック欄解析 + 採用判定
    println!("\n=== ステップ10: セル切り出し + 採用判定 ===");
    let cells_dir = output_dir.join("09_cells");
    cell::extract_and_judge(&cyan_removed, &cells_dir)?;

    println!("\n=== パイプライン完了 ===");
    Ok(())
}

/// 補正後画像の左下30%領域からQRを読み取る
fn read_qr_from_corrected(img: &RgbaImage, output_dir: &Path) -> Result<String, String> {
    let w = img.width();
    let h = img.height();

    // 左下30%領域
    let region_w = (w as f64 * 0.3) as u32;
    let region_h = (h as f64 * 0.3) as u32;
    let x0 = 0u32;
    let y0 = h - region_h;

    let mut region = GrayImage::new(region_w, region_h);
    for dy in 0..region_h {
        for dx in 0..region_w {
            let sx = x0 + dx;
            let sy = y0 + dy;
            if sx < w && sy < h {
                let p = img.get_pixel(sx, sy);
                let gray = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
                region.put_pixel(dx, dy, image::Luma([gray]));
            }
        }
    }

    // QR検出領域を保存
    let mut qr_region_img = RgbaImage::new(region_w, region_h);
    for dy in 0..region_h {
        for dx in 0..region_w {
            let g = region.get_pixel(dx, dy)[0];
            qr_region_img.put_pixel(dx, dy, Rgba([g, g, g, 255]));
        }
    }
    qr_region_img
        .save(output_dir.join("06_qr_region.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 06_qr_region.png 保存完了 ({region_w}x{region_h})");

    qr::read_qr_from_gray(&region)
}

/// 影補正: 左右グレースケールバーを読み取り、期待値との差分で勾配補正
/// 注: 現在は水平方向（左→右）の線形補間のみ。Y方向の段階的補正は
/// TypeScript版の2Dグリッド補正と異なるが、デバッグツールとしては十分
fn correct_shadow(img: &RgbaImage) -> RgbaImage {
    let w = img.width();
    let h = img.height();

    // 左右バーの位置（px）
    let bar_w_px = layout::mm_to_px(layout::GRAY_BAR_STEP_SIZE).round() as u32;
    let left_x = layout::mm_to_px(layout::GRAY_BAR_LEFT_X).round() as u32;
    let right_x = layout::mm_to_px(layout::GRAY_BAR_RIGHT_X).round() as u32;
    let top_y = layout::mm_to_px(layout::GRAY_BAR_TOP_Y).round() as u32;
    let bottom_y = layout::mm_to_px(layout::GRAY_BAR_BOTTOM_Y).round() as u32;
    let total_h = bottom_y - top_y;
    let step_h = total_h / layout::GRAY_BAR_STEPS as u32;

    // 各ステップの期待値と実測値を比較
    let mut left_ratios = Vec::new();
    let mut right_ratios = Vec::new();

    for i in 0..layout::GRAY_BAR_STEPS {
        let expected = (i as f64 / layout::GRAY_BAR_STEPS as f64 * 255.0).round();
        let y_start = top_y + i as u32 * step_h;

        // 左バーの平均輝度
        let left_avg = sample_region_brightness(img, left_x, y_start, bar_w_px, step_h);
        // 右バーの平均輝度
        let right_avg = sample_region_brightness(img, right_x, y_start, bar_w_px, step_h);

        if expected > 10.0 {
            left_ratios.push(expected / left_avg.max(1.0));
            right_ratios.push(expected / right_avg.max(1.0));
        }

        println!(
            "  バーステップ[{i}]: 期待={expected:.0} 左実測={left_avg:.1} 右実測={right_avg:.1}"
        );
    }

    // 平均比率
    let left_ratio = if left_ratios.is_empty() {
        1.0
    } else {
        left_ratios.iter().sum::<f64>() / left_ratios.len() as f64
    };
    let right_ratio = if right_ratios.is_empty() {
        1.0
    } else {
        right_ratios.iter().sum::<f64>() / right_ratios.len() as f64
    };

    println!("  補正比率: 左={left_ratio:.3} 右={right_ratio:.3}");

    // 勾配補正
    let mut out = img.clone();
    for y in 0..h {
        for x in 0..w {
            let t = x as f64 / w as f64;
            let ratio = left_ratio * (1.0 - t) + right_ratio * t;
            let p = img.get_pixel(x, y);
            let r = (p[0] as f64 * ratio).clamp(0.0, 255.0) as u8;
            let g = (p[1] as f64 * ratio).clamp(0.0, 255.0) as u8;
            let b = (p[2] as f64 * ratio).clamp(0.0, 255.0) as u8;
            out.put_pixel(x, y, Rgba([r, g, b, p[3]]));
        }
    }

    out
}

/// 領域の平均輝度を計算
fn sample_region_brightness(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> f64 {
    let mut sum = 0.0f64;
    let mut count = 0u32;

    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                let p = img.get_pixel(px, py);
                let lum = p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114;
                sum += lum;
                count += 1;
            }
        }
    }

    if count > 0 {
        sum / count as f64
    } else {
        0.0
    }
}

/// シアン除去: シアンサンプルの平均色を読み取り、色距離80以内のピクセルを白化
fn remove_cyan(img: &RgbaImage) -> RgbaImage {
    // シアンサンプルの位置
    let sample_x = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
    let sample_y = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
    let sample_size = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;

    // サンプルの平均色を計算
    let mut sum_r = 0.0f64;
    let mut sum_g = 0.0f64;
    let mut sum_b = 0.0f64;
    let mut count = 0u32;

    for dy in 0..sample_size {
        for dx in 0..sample_size {
            let px = sample_x + dx;
            let py = sample_y + dy;
            if px < img.width() && py < img.height() {
                let p = img.get_pixel(px, py);
                sum_r += p[0] as f64;
                sum_g += p[1] as f64;
                sum_b += p[2] as f64;
                count += 1;
            }
        }
    }

    let avg_r = if count > 0 { sum_r / count as f64 } else { 204.0 };
    let avg_g = if count > 0 { sum_g / count as f64 } else { 255.0 };
    let avg_b = if count > 0 { sum_b / count as f64 } else { 255.0 };

    println!("  シアンサンプル平均色: R={avg_r:.1} G={avg_g:.1} B={avg_b:.1}");

    // 色距離80以内のピクセルを白化
    let threshold = 80.0f64;
    let mut out = img.clone();
    let mut removed_count = 0u64;

    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            let dr = p[0] as f64 - avg_r;
            let dg = p[1] as f64 - avg_g;
            let db = p[2] as f64 - avg_b;
            let dist = (dr * dr + dg * dg + db * db).sqrt();
            if dist < threshold {
                out.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                removed_count += 1;
            }
        }
    }

    let total = img.width() as u64 * img.height() as u64;
    println!(
        "  シアン除去: {} ピクセル ({:.1}%)",
        removed_count,
        removed_count as f64 / total as f64 * 100.0
    );

    out
}

/// 補正品質チェック: 補正後画像でマーカーを再検出し、期待座標との残差を計測
fn verify_correction_quality(corrected: &RgbaImage, output_dir: &Path) {
    let gray = image::DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    match marker::detect_markers(&binary) {
        Ok(detected) => {
            let expected = [
                (layout::MARKER_TL, "TL"),
                (layout::MARKER_TR, "TR"),
                (layout::MARKER_BL, "BL"),
                (layout::MARKER_BR, "BR"),
            ];

            let mut max_err = 0.0f64;
            let mut total_err = 0.0f64;

            for (i, (marker_def, name)) in expected.iter().enumerate() {
                let (exp_cx, exp_cy) = layout::marker_center(marker_def);
                let exp_px_x = layout::mm_to_px(exp_cx);
                let exp_px_y = layout::mm_to_px(exp_cy);

                let det_x = detected[i].cx;
                let det_y = detected[i].cy;

                let dx = det_x - exp_px_x;
                let dy = det_y - exp_px_y;
                let err = (dx * dx + dy * dy).sqrt();
                let err_mm = err / layout::mm_to_px(1.0);

                max_err = max_err.max(err);
                total_err += err;

                let status = if err_mm < 0.5 { "OK" }
                    else if err_mm < 1.0 { "注意" }
                    else { "要改善" };

                println!(
                    "  {name}: 期待({exp_px_x:.1}, {exp_px_y:.1}) 検出({det_x:.1}, {det_y:.1}) 残差={err:.1}px ({err_mm:.2}mm) [{status}]"
                );
            }

            let avg_err = total_err / 4.0;
            let avg_mm = avg_err / layout::mm_to_px(1.0);
            let max_mm = max_err / layout::mm_to_px(1.0);
            println!("  平均残差: {avg_err:.1}px ({avg_mm:.2}mm) 最大: {max_err:.1}px ({max_mm:.2}mm)");

            if max_mm > 1.0 {
                println!("  ⚠ 台形補正の精度が不十分。罫線がセルに混入する可能性あり");
            } else {
                println!("  ✓ 台形補正の精度は良好");
            }

            // 残差可視化画像
            let mut overlay = corrected.clone();
            for (i, (marker_def, _)) in expected.iter().enumerate() {
                let (exp_cx, exp_cy) = layout::marker_center(marker_def);
                let exp_x = layout::mm_to_px(exp_cx).round() as i32;
                let exp_y = layout::mm_to_px(exp_cy).round() as i32;
                let det_x = detected[i].cx.round() as i32;
                let det_y = detected[i].cy.round() as i32;
                draw_cross(&mut overlay, exp_x, exp_y, 15, Rgba([0, 255, 0, 255]));
                draw_cross(&mut overlay, det_x, det_y, 15, Rgba([255, 0, 0, 255]));
            }
            let _ = overlay.save(output_dir.join("05b_residual.png"));
            println!("  → 05b_residual.png 保存完了 (緑=期待, 赤=検出)");
        }
        Err(e) => {
            println!("  ⚠ 補正後マーカー再検出失敗: {e}");
            println!("  台形補正の精度を確認できません");
        }
    }
}

/// 中心マーカー検証: ホモグラフィー後の中心マーカー位置を確認
fn verify_center_marker(corrected: &RgbaImage) {
    let gray = image::DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    match marker::detect_center_marker(&binary) {
        Some(detected) => {
            let (exp_cx, exp_cy) = layout::center_marker_center();
            let exp_px_x = layout::mm_to_px(exp_cx);
            let exp_px_y = layout::mm_to_px(exp_cy);

            let dx = detected.cx - exp_px_x;
            let dy = detected.cy - exp_px_y;
            let err = (dx * dx + dy * dy).sqrt();
            let err_mm = err / layout::mm_to_px(1.0);

            let status = if err_mm < 0.5 { "OK" }
                else if err_mm < 1.0 { "注意" }
                else if err_mm < 3.0 { "要改善" }
                else { "レンズ歪みの可能性" };

            println!(
                "  中心マーカー: 期待({exp_px_x:.1}, {exp_px_y:.1}) 検出({:.1}, {:.1}) 残差={err:.1}px ({err_mm:.2}mm) [{status}]",
                detected.cx, detected.cy
            );

            if err_mm > 3.0 {
                println!("  ⚠ 中心の残差が大きい → ホモグラフィーでは補正できないレンズ歪み（バレル/ピンクッション）の可能性");
            }
        }
        None => {
            println!("  中心マーカー未検出（テンプレートに中心マーカーが無い可能性）");
        }
    }
}

/// 罫線の直交性を計測し、残差回転があれば微小補正する
fn apply_orthogonality_correction(img: RgbaImage, output_dir: &Path) -> RgbaImage {
    let gray = image::DynamicImage::ImageRgba8(img.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    // 複数の縦罫線で角度を計測（ボディ領域の列境界）
    let col_xs: Vec<f64> = (0..=layout::COLS)
        .map(|c| layout::BODY_START_X + c as f64 * layout::COL_WIDTH)
        .collect();

    let y_top = layout::mm_to_px(layout::BODY_START_Y + 2.0).round() as u32;
    let y_bottom = layout::mm_to_px(layout::BODY_START_Y + 10.0 * layout::ROW_HEIGHT + 2.0).round() as u32;

    let mut angles = Vec::new();

    for &col_x_mm in &col_xs {
        let expected_x = layout::mm_to_px(col_x_mm).round() as i32;
        let top_x = find_grid_line_x(&binary, expected_x, y_top, 30);
        let bottom_x = find_grid_line_x(&binary, expected_x, y_bottom, 30);

        if let (Some(tx), Some(bx)) = (top_x, bottom_x) {
            let dx = bx as f64 - tx as f64;
            let dy = y_bottom as f64 - y_top as f64;
            let angle_deg = (dx / dy).atan().to_degrees();
            angles.push(angle_deg);
            println!(
                "  縦罫線 x={col_x_mm:.0}mm: top_x={tx} bottom_x={bx} 角度={angle_deg:.3}°"
            );
        }
    }

    if angles.is_empty() {
        println!("  罫線検出できず → 直交性補正スキップ");
        return img;
    }

    // 中央値を使う（外れ値に強い）
    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median_angle = angles[angles.len() / 2];
    println!("  残差回転角度（中央値）: {median_angle:.3}°");

    if median_angle.abs() < 0.05 {
        println!("  ✓ 直交性は良好（補正不要）");
        return img;
    }

    println!("  → {:.3}° の微小回転補正を適用", -median_angle);
    let corrected = rotate_small_angle(&img, median_angle);
    let _ = corrected.save(output_dir.join("05c_orthogonal.png"));
    println!("  → 05c_orthogonal.png 保存完了");

    corrected
}

/// 期待X位置付近で縦罫線（黒ピクセル）のX座標を探す
/// ±2px の垂直バンドを走査して、サブピクセル位置ずれに対応
fn find_grid_line_x(binary: &GrayImage, expected_x: i32, y: u32, search_range: i32) -> Option<i32> {
    let mut best_x = None;
    let mut min_dist = search_range + 1;

    for dy in -2i32..=2 {
        let sy = (y as i32 + dy).max(0) as u32;
        if sy >= binary.height() { continue; }

        for dx in -search_range..=search_range {
            let x = expected_x + dx;
            if x < 0 || x as u32 >= binary.width() {
                continue;
            }
            if binary.get_pixel(x as u32, sy)[0] == 0 {
                if dx.abs() < min_dist {
                    min_dist = dx.abs();
                    best_x = Some(x);
                }
            }
        }
    }
    best_x
}

/// 微小角度の回転補正（ページ中心を基準に回転）
fn rotate_small_angle(img: &RgbaImage, degrees: f64) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let rad = -degrees.to_radians();
    let cos_a = rad.cos();
    let sin_a = rad.sin();

    let mut out = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));

    for y in 0..h {
        for x in 0..w {
            let dx = x as f64 - cx;
            let dy = y as f64 - cy;
            let src_x = cx + dx * cos_a - dy * sin_a;
            let src_y = cy + dx * sin_a + dy * cos_a;
            let pixel = perspective::sample_bilinear(img, src_x, src_y);
            out.put_pixel(x, y, pixel);
        }
    }

    out
}

fn draw_cross(img: &mut RgbaImage, cx: i32, cy: i32, size: i32, color: Rgba<u8>) {
    for d in -size..=size {
        for t in [-1i32, 0, 1] {
            let px = cx + d;
            let py = cy + t;
            if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                img.put_pixel(px as u32, py as u32, color);
            }
            let px = cx + t;
            let py = cy + d;
            if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                img.put_pixel(px as u32, py as u32, color);
            }
        }
    }
}
