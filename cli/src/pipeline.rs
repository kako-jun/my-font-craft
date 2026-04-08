/// 画像処理パイプライン（process サブコマンド）
use image::{DynamicImage, GrayImage, RgbaImage, Rgba};
use std::path::Path;
use crate::{layout, marker, perspective, qr, cell};

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

    // ステップ5: ページ四隅外挿
    println!("\n=== ステップ5: ページ四隅外挿 ===");
    let page_corners = perspective::extrapolate_page_corners(&oriented_markers);

    // ステップ6: 射影変換
    println!("\n=== ステップ6: 射影変換 ===");
    let corrected = perspective::bilinear_warp(&oriented_img, &page_corners);
    corrected
        .save(output_dir.join("05_corrected.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    println!("  → 05_corrected.png 保存完了");

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
