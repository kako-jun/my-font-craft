/// 画像処理パイプライン（process サブコマンド + WASM用エントリポイント）
use image::{DynamicImage, GrayImage, RgbaImage, Rgba};
use serde::{Serialize, Deserialize};
use crate::{layout, marker, perspective, qr, cell};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

// ── WASM公開用の結果型 ──

/// 1セルの処理結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedCell {
    pub row: usize,
    pub col: usize,
    pub char_index: Option<usize>,
    pub is_empty: bool,
    pub adopted: bool,
    pub cell_index: usize,
    pub image_data: Vec<u8>,  // RGBA生データ
    pub width: u32,
    pub height: u32,
}

/// パイプライン全体の処理結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResult {
    pub page_number: Option<u32>,
    pub total_pages: Option<u32>,
    pub cells: Vec<ProcessedCell>,
    pub corrected_image: Vec<u8>,  // 補正後画像のRGBA
    pub corrected_width: u32,
    pub corrected_height: u32,
}

// ── DPI算出 ──

/// マーカー間のピクセル距離から実効DPIを算出
fn estimate_dpi(markers: &[marker::DetectedMarker; 4]) -> f64 {
    // markers[0]=TL, markers[1]=TR, markers[2]=BL, markers[3]=BR
    // TL→TR: 198.0mm (水平)
    let h_px = ((markers[1].cx - markers[0].cx).powi(2) + (markers[1].cy - markers[0].cy).powi(2)).sqrt();
    let h_dpi = h_px / 198.0 * 25.4;

    // TL→BL: 286.0mm (垂直)
    let v_px = ((markers[2].cx - markers[0].cx).powi(2) + (markers[2].cy - markers[0].cy).powi(2)).sqrt();
    let v_dpi = v_px / 286.0 * 25.4;

    let avg_dpi = (h_dpi + v_dpi) / 2.0;

    log!("  水平DPI: {h_dpi:.1} (TL→TR: {h_px:.1}px / 198.0mm)");
    log!("  垂直DPI: {v_dpi:.1} (TL→BL: {v_px:.1}px / 286.0mm)");
    log!("  平均DPI: {avg_dpi:.1}");

    // アスペクト比異常の警告
    let ratio = h_dpi / v_dpi;
    if ratio < 0.9 || ratio > 1.1 {
        log!("  ⚠ アスペクト比異常: 水平/垂直 = {ratio:.3} (期待: ≈1.0)");
    }

    avg_dpi
}

/// DPIに基づいて警告/エラーを判定
fn check_dpi(dpi: f64) -> Result<(), String> {
    if dpi < 150.0 {
        return Err(format!("解像度が低すぎます ({dpi:.0} DPI)。もう少し近づけて撮影してください（推奨: 300DPI以上）"));
    }
    if dpi < 250.0 {
        log!("  ⚠ 解像度が低めです ({dpi:.0} DPI)。処理は続行しますが品質が低下する可能性があります");
    } else {
        log!("  ✓ 解像度: {dpi:.0} DPI (OK)");
    }
    Ok(())
}

// ── CLI用パイプライン（ファイルI/O付き） ──

/// CLI用パイプラインを実行（ファイル読み込み・デバッグ画像保存付き）
#[cfg(not(target_arch = "wasm32"))]
pub fn run_pipeline(image_path: &Path, output_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;

    // ステップ1: 画像読み込み
    log!("\n=== ステップ1: 画像読み込み ===");
    let img = image::open(image_path)
        .map_err(|e| format!("画像読み込みエラー: {e}"))?;
    let rgba = img.to_rgba8();
    log!("  画像サイズ: {}x{}", rgba.width(), rgba.height());
    rgba.save(output_dir.join("01_input.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 01_input.png 保存完了");

    // ステップ2: 二値化
    log!("\n=== ステップ2: 二値化 ===");
    let gray = DynamicImage::ImageRgba8(rgba.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    log!("  大津の閾値: {threshold}");
    let mut binary = marker::binarize(&gray, threshold);
    binary
        .save(output_dir.join("02_binary.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 02_binary.png 保存完了");

    // ステップ2.5: 背景除去（実写画像対応）
    log!("\n=== ステップ2.5: 背景除去 ===");
    marker::mask_border_background(&mut binary);
    binary
        .save(output_dir.join("02b_masked.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 02b_masked.png 保存完了（境界接触領域を白化）");

    // ステップ3: マーカー検出
    log!("\n=== ステップ3: マーカー検出 ===");
    let markers = marker::detect_markers(&binary, &gray)?;
    let marker_img = marker::draw_marker_overlay(&rgba, &markers);
    marker_img
        .save(output_dir.join("03_markers.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 03_markers.png 保存完了");

    // ステップ4: 向き検出
    log!("\n=== ステップ4: 向き検出 ===");
    let (tl_index, rotation) = marker::detect_orientation(&binary, &markers)?;

    let (oriented_img, oriented_markers) = if rotation != 0 {
        log!("  画像を{rotation}°回転します");
        let rotated = marker::rotate_image(&rgba, rotation);
        let reordered = marker::reorder_markers(&markers, tl_index, rotation, rgba.width(), rgba.height());
        (rotated, reordered)
    } else {
        (rgba.clone(), markers.clone())
    };

    oriented_img
        .save(output_dir.join("04_oriented.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 04_oriented.png 保存完了");

    // ステップ4.5: DPI算出
    log!("\n=== ステップ4.5: DPI算出 ===");
    let dpi = estimate_dpi(&oriented_markers);
    check_dpi(dpi)?;

    // ステップ5+6: マーカー4点から直接ホモグラフィー変換（外挿廃止）+ 反復収束
    log!("\n=== ステップ5+6: マーカー直接ホモグラフィー変換 ===");
    let mut corrected = perspective::homography_warp_from_markers(&oriented_img, &oriented_markers);

    // 反復台形補正（最大3回）
    let max_iterations = 3;
    let residual_threshold_mm = 1.0;

    for iteration in 0..max_iterations {
        log!("\n=== ステップ6.5: 補正品質チェック (反復{}) ===", iteration + 1);
        match verify_correction_quality_cli(&corrected, output_dir) {
            Some((max_residual_mm, re_detected)) => {
                if max_residual_mm <= residual_threshold_mm {
                    log!("  ✓ 残差 {max_residual_mm:.2}mm — 収束");
                    break;
                }
                if iteration == max_iterations - 1 {
                    log!("  ⚠ {max_iterations}回で収束せず（残差 {max_residual_mm:.2}mm）");
                    break;
                }
                log!("  反復{}: 残差 {max_residual_mm:.2}mm — 再補正", iteration + 1);
                corrected = perspective::homography_refine(&corrected, &re_detected);
            }
            None => {
                log!("  マーカー再検出失敗 — 反復中断");
                break;
            }
        }
    }

    corrected
        .save(output_dir.join("05_corrected.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 05_corrected.png 保存完了");

    // ステップ6.6: 中心マーカー検証
    log!("\n=== ステップ6.6: 中心マーカー検証 ===");
    verify_center_marker(&corrected);

    // ステップ6.7: 罫線直交性チェック＋微小回転補正
    log!("\n=== ステップ6.7: 罫線直交性チェック ===");
    let corrected = apply_orthogonality_correction_cli(corrected, output_dir);

    // ステップ7: QR読み取り
    log!("\n=== ステップ7: QR読み取り ===");
    let qr_result = read_qr_from_corrected_cli(&corrected, output_dir);
    match &qr_result {
        Ok(data) => log!("  QRデータ: {data}"),
        Err(e) => log!("  QR読み取り失敗（続行）: {e}"),
    }

    // ステップ7.5: ホワイトバランス補正
    log!("\n=== ステップ7.5: ホワイトバランス補正 ===");
    let wb_corrected = correct_white_balance(&corrected);
    wb_corrected
        .save(output_dir.join("07a_white_balanced.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 07a_white_balanced.png 保存完了");

    // ステップ8: 影補正
    log!("\n=== ステップ8: 影補正 ===");
    let shadow_corrected = correct_shadow(&wb_corrected);
    shadow_corrected
        .save(output_dir.join("07_shadow_corrected.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 07_shadow_corrected.png 保存完了");

    // ステップ9: シアン除去
    log!("\n=== ステップ9: シアン除去 ===");
    let cyan_removed = remove_cyan(&shadow_corrected);
    cyan_removed
        .save(output_dir.join("08_cyan_removed.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 08_cyan_removed.png 保存完了");

    // ステップ9.5: 罫線残骸除去
    log!("\n=== ステップ9.5: 罫線残骸除去 ===");
    let grid_removed = erase_grid_lines(&cyan_removed);
    grid_removed
        .save(output_dir.join("08b_grid_removed.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 08b_grid_removed.png 保存完了");

    // ステップ10: セル切り出し + チェック欄解析 + 採用判定
    log!("\n=== ステップ10: セル切り出し + 採用判定 ===");
    let cells_dir = output_dir.join("09_cells");
    cell::extract_and_judge(&grid_removed, &cells_dir)?;

    log!("\n=== パイプライン完了 ===");
    Ok(())
}

// ── WASM用パイプライン（ファイルI/Oなし） ──

/// WASM用: バイト列から画像を処理して結果を返す
pub fn process_image_bytes(bytes: &[u8]) -> Result<ProcessResult, String> {
    // ステップ1: 画像デコード
    log!("=== ステップ1: 画像デコード ===");
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("画像デコードエラー: {e}"))?;
    let rgba = img.to_rgba8();
    log!("  画像サイズ: {}x{}", rgba.width(), rgba.height());

    // ステップ2: 二値化
    log!("=== ステップ2: 二値化 ===");
    let gray = DynamicImage::ImageRgba8(rgba.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    log!("  大津の閾値: {threshold}");
    let mut binary = marker::binarize(&gray, threshold);

    // ステップ2.5: 背景除去（実写画像対応）
    log!("=== ステップ2.5: 背景除去 ===");
    marker::mask_border_background(&mut binary);
    log!("  境界接触領域を白化");

    // ステップ3: マーカー検出
    log!("=== ステップ3: マーカー検出 ===");
    let markers = marker::detect_markers(&binary, &gray)?;

    // ステップ4: 向き検出
    log!("=== ステップ4: 向き検出 ===");
    let (tl_index, rotation) = marker::detect_orientation(&binary, &markers)?;

    let (oriented_img, oriented_markers) = if rotation != 0 {
        log!("  画像を{rotation}°回転します");
        let rotated = marker::rotate_image(&rgba, rotation);
        let reordered = marker::reorder_markers(&markers, tl_index, rotation, rgba.width(), rgba.height());
        (rotated, reordered)
    } else {
        (rgba.clone(), markers.clone())
    };

    // ステップ4.5: DPI算出
    log!("=== ステップ4.5: DPI算出 ===");
    let dpi = estimate_dpi(&oriented_markers);
    check_dpi(dpi)?;

    // ステップ5+6: ホモグラフィー変換 + 反復収束
    log!("=== ステップ5+6: ホモグラフィー変換 ===");
    let mut corrected = perspective::homography_warp_from_markers(&oriented_img, &oriented_markers);

    let max_iterations = 3;
    let residual_threshold_mm = 1.0;

    for iteration in 0..max_iterations {
        log!("=== 補正品質チェック (反復{}) ===", iteration + 1);
        match verify_correction_quality_wasm(&corrected) {
            Some((max_residual_mm, re_detected)) => {
                if max_residual_mm <= residual_threshold_mm {
                    log!("  残差 {max_residual_mm:.2}mm — 収束");
                    break;
                }
                if iteration == max_iterations - 1 {
                    log!("  {max_iterations}回で収束せず（残差 {max_residual_mm:.2}mm）");
                    break;
                }
                log!("  反復{}: 残差 {max_residual_mm:.2}mm — 再補正", iteration + 1);
                corrected = perspective::homography_refine(&corrected, &re_detected);
            }
            None => {
                log!("  マーカー再検出失敗 — 反復中断");
                break;
            }
        }
    }

    // ステップ6.6: 中心マーカー検証
    log!("=== 中心マーカー検証 ===");
    verify_center_marker(&corrected);

    // ステップ6.7: 罫線直交性チェック
    log!("=== 罫線直交性チェック ===");
    let corrected = apply_orthogonality_correction_wasm(corrected);

    // ステップ7: QR読み取り
    log!("=== QR読み取り ===");
    let (page_number, total_pages) = match read_qr_from_corrected_wasm(&corrected) {
        Ok(data) => {
            log!("  QRデータ: {data}");
            parse_qr_page_info(&data)
        }
        Err(e) => {
            log!("  QR読み取り失敗（続行）: {e}");
            (None, None)
        }
    };

    // ステップ7.5: ホワイトバランス補正
    log!("=== ホワイトバランス補正 ===");
    let wb_corrected = correct_white_balance(&corrected);

    // ステップ8: 影補正
    log!("=== 影補正 ===");
    let shadow_corrected = correct_shadow(&wb_corrected);

    // ステップ9: シアン除去
    log!("=== シアン除去 ===");
    let cyan_removed = remove_cyan(&shadow_corrected);

    // ステップ9.5: 罫線残骸除去
    log!("=== 罫線残骸除去 ===");
    let grid_removed = erase_grid_lines(&cyan_removed);

    // ステップ10: セル切り出し + 採用判定
    log!("=== セル切り出し + 採用判定 ===");
    let char_results = cell::extract_and_judge_in_memory(&grid_removed)?;

    // 結果をProcessResult に変換
    let corrected_width = grid_removed.width();
    let corrected_height = grid_removed.height();
    let corrected_image = grid_removed.into_raw();

    let mut cells = Vec::new();
    for cr in &char_results {
        let char_index = layout::grid_to_char_index(cr.row, cr.col);
        for slot in &cr.slots {
            let cell_img = cell::extract_cell_image(&shadow_corrected, cr.row, cr.col, slot.cell_index);
            let width = cell_img.width();
            let height = cell_img.height();
            let image_data = cell_img.into_raw();

            let adopted = cr.adopted.contains(&slot.cell_index);

            cells.push(ProcessedCell {
                row: cr.row,
                col: cr.col,
                char_index,
                is_empty: slot.is_empty,
                adopted,
                cell_index: slot.cell_index,
                image_data,
                width,
                height,
            });
        }
    }

    Ok(ProcessResult {
        page_number,
        total_pages,
        cells,
        corrected_image,
        corrected_width,
        corrected_height,
    })
}

/// QRデータからページ情報を抽出
fn parse_qr_page_info(data: &str) -> (Option<u32>, Option<u32>) {
    // QRデータ形式: "mfc:1/3" など（ページ番号/全ページ数）
    if let Some(stripped) = data.strip_prefix("mfc:") {
        let parts: Vec<&str> = stripped.split('/').collect();
        if parts.len() == 2 {
            let page = parts[0].parse().ok();
            let total = parts[1].parse().ok();
            return (page, total);
        }
    }
    (None, None)
}

// ── 共通処理関数（CLI/WASM両方で使用） ──

/// 補正品質チェック（画像保存なし版 — WASM用）
fn verify_correction_quality_wasm(corrected: &RgbaImage) -> Option<(f64, [marker::DetectedMarker; 4])> {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    match marker::detect_markers(&binary, &gray) {
        Ok(detected) => {
            let expected = [
                (layout::MARKER_TL, "TL"),
                (layout::MARKER_TR, "TR"),
                (layout::MARKER_BL, "BL"),
                (layout::MARKER_BR, "BR"),
            ];

            let mut max_err = 0.0f64;

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

                log!(
                    "  {name}: 期待({exp_px_x:.1}, {exp_px_y:.1}) 検出({det_x:.1}, {det_y:.1}) 残差={err:.1}px ({err_mm:.2}mm)"
                );
            }

            let max_mm = max_err / layout::mm_to_px(1.0);
            Some((max_mm, detected))
        }
        Err(e) => {
            log!("  補正後マーカー再検出失敗: {e}");
            None
        }
    }
}

/// 補正品質チェック（CLI用 — デバッグ画像保存付き）
#[cfg(not(target_arch = "wasm32"))]
fn verify_correction_quality_cli(corrected: &RgbaImage, output_dir: &Path) -> Option<(f64, [marker::DetectedMarker; 4])> {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    match marker::detect_markers(&binary, &gray) {
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

                log!(
                    "  {name}: 期待({exp_px_x:.1}, {exp_px_y:.1}) 検出({det_x:.1}, {det_y:.1}) 残差={err:.1}px ({err_mm:.2}mm) [{status}]"
                );
            }

            let avg_err = total_err / 4.0;
            let avg_mm = avg_err / layout::mm_to_px(1.0);
            let max_mm = max_err / layout::mm_to_px(1.0);
            log!("  平均残差: {avg_err:.1}px ({avg_mm:.2}mm) 最大: {max_err:.1}px ({max_mm:.2}mm)");

            if max_mm > 1.0 {
                log!("  ⚠ 台形補正の精度が不十分。罫線がセルに混入する可能性あり");
            } else {
                log!("  ✓ 台形補正の精度は良好");
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
            log!("  → 05b_residual.png 保存完了 (緑=期待, 赤=検出)");

            Some((max_mm, detected))
        }
        Err(e) => {
            log!("  ⚠ 補正後マーカー再検出失敗: {e}");
            log!("  台形補正の精度を確認できません");
            None
        }
    }
}

/// 中心マーカー検証（CLI/WASM共通）
fn verify_center_marker(corrected: &RgbaImage) {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
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

            log!(
                "  中心マーカー: 期待({exp_px_x:.1}, {exp_px_y:.1}) 検出({:.1}, {:.1}) 残差={err:.1}px ({err_mm:.2}mm) [{status}]",
                detected.cx, detected.cy
            );

            if err_mm > 3.0 {
                log!("  ⚠ 中心の残差が大きい → ホモグラフィーでは補正できないレンズ歪み（バレル/ピンクッション）の可能性");
            }
        }
        None => {
            log!("  中心マーカー未検出（テンプレートに中心マーカーが無い可能性）");
        }
    }
}

/// QR読み取り（WASM用 — ファイル保存なし）
fn read_qr_from_corrected_wasm(img: &RgbaImage) -> Result<String, String> {
    let w = img.width();
    let h = img.height();

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

    qr::read_qr_from_gray(&region)
}

/// QR読み取り（CLI用 — デバッグ画像保存付き）
#[cfg(not(target_arch = "wasm32"))]
fn read_qr_from_corrected_cli(img: &RgbaImage, output_dir: &Path) -> Result<String, String> {
    let w = img.width();
    let h = img.height();

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
    log!("  → 06_qr_region.png 保存完了 ({region_w}x{region_h})");

    qr::read_qr_from_gray(&region)
}

/// 罫線直交性チェック＋微小回転補正（WASM用 — ファイル保存なし）
fn apply_orthogonality_correction_wasm(img: RgbaImage) -> RgbaImage {
    let gray = DynamicImage::ImageRgba8(img.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    let median_angle = measure_grid_angle(&binary);

    if let Some(angle) = median_angle {
        if angle.abs() < 0.05 {
            log!("  直交性は良好（補正不要）");
            return img;
        }
        if angle.abs() > 5.0 {
            log!("  ⚠ 残留角度 {:.2}° が大きすぎます — 台形補正が失敗している可能性があります", angle.abs());
        }
        log!("  → {:.3}° の微小回転補正を適用", -angle);
        rotate_small_angle(&img, angle)
    } else {
        log!("  罫線検出できず → 直交性補正スキップ");
        img
    }
}

/// 罫線直交性チェック＋微小回転補正（CLI用 — デバッグ画像保存付き）
#[cfg(not(target_arch = "wasm32"))]
fn apply_orthogonality_correction_cli(img: RgbaImage, output_dir: &Path) -> RgbaImage {
    let gray = DynamicImage::ImageRgba8(img.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    let median_angle = measure_grid_angle(&binary);

    if let Some(angle) = median_angle {
        if angle.abs() < 0.05 {
            log!("  ✓ 直交性は良好（補正不要）");
            return img;
        }
        if angle.abs() > 5.0 {
            log!("  ⚠ 残留角度 {:.2}° が大きすぎます — 台形補正が失敗している可能性があります", angle.abs());
        }
        log!("  → {:.3}° の微小回転補正を適用", -angle);
        let corrected = rotate_small_angle(&img, angle);
        let _ = corrected.save(output_dir.join("05c_orthogonal.png"));
        log!("  → 05c_orthogonal.png 保存完了");
        corrected
    } else {
        log!("  罫線検出できず → 直交性補正スキップ");
        img
    }
}

/// 罫線角度の中央値を計測（共通ロジック）
fn measure_grid_angle(binary: &GrayImage) -> Option<f64> {
    let col_xs: Vec<f64> = (0..=layout::COLS)
        .map(|c| layout::BODY_START_X + c as f64 * layout::COL_WIDTH)
        .collect();

    let y_top = layout::mm_to_px(layout::BODY_START_Y + 2.0).round() as u32;
    let y_bottom = layout::mm_to_px(layout::BODY_START_Y + 10.0 * layout::ROW_HEIGHT + 2.0).round() as u32;

    let mut angles = Vec::new();

    for &col_x_mm in &col_xs {
        let expected_x = layout::mm_to_px(col_x_mm).round() as i32;
        let top_x = find_grid_line_x(binary, expected_x, y_top, 30);
        let bottom_x = find_grid_line_x(binary, expected_x, y_bottom, 30);

        if let (Some(tx), Some(bx)) = (top_x, bottom_x) {
            let dx = bx as f64 - tx as f64;
            let dy = y_bottom as f64 - y_top as f64;
            let angle_deg = (dx / dy).atan().to_degrees();
            angles.push(angle_deg);
            log!(
                "  縦罫線 x={col_x_mm:.0}mm: top_x={tx} bottom_x={bx} 角度={angle_deg:.3}°"
            );
        }
    }

    if angles.is_empty() {
        return None;
    }

    angles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = angles[angles.len() / 2];
    log!("  残差回転角度（中央値）: {median:.3}°");
    Some(median)
}

// ── 共通ヘルパー関数 ──

/// 影補正: 左右グレースケールバーを読み取り、期待値との差分で勾配補正
fn correct_shadow(img: &RgbaImage) -> RgbaImage {
    let w = img.width();
    let h = img.height();

    let bar_w_px = layout::mm_to_px(layout::GRAY_BAR_STEP_SIZE).round() as u32;
    let left_x = layout::mm_to_px(layout::GRAY_BAR_LEFT_X).round() as u32;
    let right_x = layout::mm_to_px(layout::GRAY_BAR_RIGHT_X).round() as u32;
    let top_y = layout::mm_to_px(layout::GRAY_BAR_TOP_Y).round() as u32;
    let bottom_y = layout::mm_to_px(layout::GRAY_BAR_BOTTOM_Y).round() as u32;
    let total_h = bottom_y - top_y;
    let step_h = total_h / layout::GRAY_BAR_STEPS as u32;

    let mut left_ratios = Vec::new();
    let mut right_ratios = Vec::new();

    for i in 0..layout::GRAY_BAR_STEPS {
        let expected = (i as f64 / layout::GRAY_BAR_STEPS as f64 * 255.0).round();
        let y_start = top_y + i as u32 * step_h;

        let left_avg = sample_region_brightness(img, left_x, y_start, bar_w_px, step_h);
        let right_avg = sample_region_brightness(img, right_x, y_start, bar_w_px, step_h);

        if expected > 10.0 {
            left_ratios.push(expected / left_avg.max(1.0));
            right_ratios.push(expected / right_avg.max(1.0));
        }

        log!(
            "  バーステップ[{i}]: 期待={expected:.0} 左実測={left_avg:.1} 右実測={right_avg:.1}"
        );
    }

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

    log!("  補正比率: 左={left_ratio:.3} 右={right_ratio:.3}");

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

/// 領域のRGB各チャネル平均を計算
fn sample_region_rgb(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> (f64, f64, f64) {
    let mut sum_r = 0.0f64;
    let mut sum_g = 0.0f64;
    let mut sum_b = 0.0f64;
    let mut count = 0u32;

    for dy in 0..h {
        for dx in 0..w {
            let px = x + dx;
            let py = y + dy;
            if px < img.width() && py < img.height() {
                let p = img.get_pixel(px, py);
                sum_r += p[0] as f64;
                sum_g += p[1] as f64;
                sum_b += p[2] as f64;
                count += 1;
            }
        }
    }

    if count > 0 {
        (sum_r / count as f64, sum_g / count as f64, sum_b / count as f64)
    } else {
        (0.0, 0.0, 0.0)
    }
}

/// ホワイトバランス補正: グレースケールバーの明部ステップからRGB補正係数を算出
fn correct_white_balance(img: &RgbaImage) -> RgbaImage {
    let bar_w_px = layout::mm_to_px(layout::GRAY_BAR_STEP_SIZE).round() as u32;
    let left_x = layout::mm_to_px(layout::GRAY_BAR_LEFT_X).round() as u32;
    let right_x = layout::mm_to_px(layout::GRAY_BAR_RIGHT_X).round() as u32;
    let top_y = layout::mm_to_px(layout::GRAY_BAR_TOP_Y).round() as u32;
    let bottom_y = layout::mm_to_px(layout::GRAY_BAR_BOTTOM_Y).round() as u32;
    let total_h = bottom_y - top_y;
    let step_h = total_h / layout::GRAY_BAR_STEPS as u32;

    // 上位3ステップ（最明部付近）を平均して色温度バイアスを推定
    // 単一ステップだとノイズや局所的な影に弱いため複数ステップで安定化
    let calibration_steps = 3usize;
    let start_step = layout::GRAY_BAR_STEPS - calibration_steps;

    let mut total_coeff_r = 0.0f64;
    let mut total_coeff_g = 0.0f64;
    let mut total_coeff_b = 0.0f64;
    let mut valid_steps = 0usize;

    for step_index in start_step..layout::GRAY_BAR_STEPS {
        let expected = (step_index as f64 / layout::GRAY_BAR_STEPS as f64 * 255.0).round();
        let y_start = top_y + step_index as u32 * step_h;

        let (l_r, l_g, l_b) = sample_region_rgb(img, left_x, y_start, bar_w_px, step_h);
        let (r_r, r_g, r_b) = sample_region_rgb(img, right_x, y_start, bar_w_px, step_h);

        // サンプリング領域が画像外の場合はスキップ
        if (l_r + l_g + l_b) < 1.0 && (r_r + r_g + r_b) < 1.0 {
            log!("  ⚠ バーステップ[{step_index}]: サンプリング領域が無効 — スキップ");
            continue;
        }

        log!("  バーステップ[{step_index}]: 期待={expected:.0} 左RGB=({l_r:.1}, {l_g:.1}, {l_b:.1}) 右RGB=({r_r:.1}, {r_g:.1}, {r_b:.1})");

        // 左右バーの平均RGB
        let avg_r = ((l_r + r_r) / 2.0).max(1.0);
        let avg_g = ((l_g + r_g) / 2.0).max(1.0);
        let avg_b = ((l_b + r_b) / 2.0).max(1.0);

        total_coeff_r += expected / avg_r;
        total_coeff_g += expected / avg_g;
        total_coeff_b += expected / avg_b;
        valid_steps += 1;
    }

    if valid_steps == 0 {
        log!("  ⚠ 有効なキャリブレーションステップなし — スキップ");
        return img.clone();
    }

    let coeff_r = total_coeff_r / valid_steps as f64;
    let coeff_g = total_coeff_g / valid_steps as f64;
    let coeff_b = total_coeff_b / valid_steps as f64;

    log!("  WB補正係数: R={coeff_r:.3} G={coeff_g:.3} B={coeff_b:.3} ({valid_steps}ステップ平均)");

    // 補正係数が極端な場合はスキップ
    if coeff_r < 0.5 || coeff_r > 2.0 || coeff_g < 0.5 || coeff_g > 2.0 || coeff_b < 0.5 || coeff_b > 2.0 {
        log!("  ⚠ 補正係数が極端なためスキップ");
        return img.clone();
    }

    let mut out = img.clone();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            let r = (p[0] as f64 * coeff_r).clamp(0.0, 255.0) as u8;
            let g = (p[1] as f64 * coeff_g).clamp(0.0, 255.0) as u8;
            let b = (p[2] as f64 * coeff_b).clamp(0.0, 255.0) as u8;
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

/// シアン除去: シアンサンプルの平均色を読み取り、紙色との差が十分あれば色距離基準で白化
/// 実写画像ではカメラのホワイトバランスで紙全体がシアンがかるため、
/// サンプル色≒紙色の場合はスキップする
fn remove_cyan(img: &RgbaImage) -> RgbaImage {
    let sample_x = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
    let sample_y = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
    let sample_size = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;

    // シアンサンプル色を取得
    let (cyan_r, cyan_g, cyan_b) = sample_region_rgb(img, sample_x, sample_y, sample_size, sample_size);

    // 紙色をセル内の空白エリアからサンプリング（ページ中央付近の既知の白領域）
    let paper_x = layout::mm_to_px(100.0).round() as u32;
    let paper_y = layout::mm_to_px(50.0).round() as u32;
    let paper_size = layout::mm_to_px(5.0).round() as u32;
    let (paper_r, paper_g, paper_b) = sample_region_rgb(img, paper_x, paper_y, paper_size, paper_size);

    log!("  シアンサンプル平均色: R={cyan_r:.1} G={cyan_g:.1} B={cyan_b:.1}");
    log!("  紙色（参照）: R={paper_r:.1} G={paper_g:.1} B={paper_b:.1}");

    // シアンサンプルと紙色の距離を計算
    let dr = cyan_r - paper_r;
    let dg = cyan_g - paper_g;
    let db = cyan_b - paper_b;
    let cyan_paper_dist = (dr * dr + dg * dg + db * db).sqrt();
    log!("  シアン-紙間距離: {cyan_paper_dist:.1}");

    // 距離が小さすぎる場合（シアンが紙と区別できない場合）はスキップ
    // 実写画像では環境光の影響でシアンサンプルと紙の色差が小さくなる
    // 純シアン(R=204,G=255,B=255)と白(255,255,255)の距離≈72なので、
    // 60未満なら実写の色被りと判断してスキップ
    if cyan_paper_dist < 60.0 {
        log!("  ⚠ シアンサンプルが紙色と近似 — シアン除去をスキップ");
        return img.clone();
    }

    let threshold = 80.0f64;
    let mut out = img.clone();
    let mut removed_count = 0u64;

    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            let dr = p[0] as f64 - cyan_r;
            let dg = p[1] as f64 - cyan_g;
            let db = p[2] as f64 - cyan_b;
            let dist = (dr * dr + dg * dg + db * db).sqrt();
            if dist < threshold {
                out.put_pixel(x, y, Rgba([255, 255, 255, 255]));
                removed_count += 1;
            }
        }
    }

    let total = img.width() as u64 * img.height() as u64;
    log!(
        "  シアン除去: {} ピクセル ({:.1}%)",
        removed_count,
        removed_count as f64 / total as f64 * 100.0
    );

    out
}

/// 罫線残骸除去: レイアウト定数から罫線位置を算出し、±2px を白で塗りつぶす
fn erase_grid_lines(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    let line_margin = 2u32;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                erase_horizontal_line(&mut out, mm_x, mm_y, layout::CELL_SIZE, line_margin);
                erase_horizontal_line(&mut out, mm_x, mm_y + layout::CELL_SIZE, layout::CELL_SIZE, line_margin);
                erase_vertical_line(&mut out, mm_x, mm_y, layout::CELL_SIZE, line_margin);
                erase_vertical_line(&mut out, mm_x + layout::CELL_SIZE, mm_y, layout::CELL_SIZE, line_margin);

                let inner_offset = (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0;
                let ix = mm_x + inner_offset;
                let iy = mm_y + inner_offset;
                erase_horizontal_line(&mut out, ix, iy, layout::INNER_SIZE, line_margin);
                erase_horizontal_line(&mut out, ix, iy + layout::INNER_SIZE, layout::INNER_SIZE, line_margin);
                erase_vertical_line(&mut out, ix, iy, layout::INNER_SIZE, line_margin);
                erase_vertical_line(&mut out, ix + layout::INNER_SIZE, iy, layout::INNER_SIZE, line_margin);

                let check_y = mm_y + layout::CELL_SIZE;
                erase_horizontal_line(&mut out, mm_x, check_y + layout::CHECK_HEIGHT, layout::CELL_SIZE, line_margin);
                erase_vertical_line(&mut out, mm_x, check_y, layout::CHECK_HEIGHT, line_margin);
                erase_vertical_line(&mut out, mm_x + layout::CELL_SIZE, check_y, layout::CHECK_HEIGHT, line_margin);
            }

            let (sx, sy) = layout::get_sample_position(row, col);
            erase_horizontal_line(&mut out, sx, sy, layout::SAMPLE_WIDTH, line_margin);
            erase_horizontal_line(&mut out, sx, sy + layout::CELL_SIZE, layout::SAMPLE_WIDTH, line_margin);
            erase_vertical_line(&mut out, sx, sy, layout::CELL_SIZE, line_margin);
            erase_vertical_line(&mut out, sx + layout::SAMPLE_WIDTH, sy, layout::CELL_SIZE, line_margin);
        }
    }

    log!("  罫線残骸除去完了");
    out
}

/// 水平罫線を白で塗りつぶす
fn erase_horizontal_line(img: &mut RgbaImage, x_mm: f64, y_mm: f64, width_mm: f64, margin_px: u32) {
    let white = Rgba([255, 255, 255, 255]);
    let x_start = layout::mm_to_px(x_mm).round() as i32;
    let y_center = layout::mm_to_px(y_mm).round() as i32;
    let w_px = layout::mm_to_px(width_mm).round() as i32;

    let y_lo = (y_center - margin_px as i32).max(0) as u32;
    let y_hi = ((y_center + margin_px as i32) as u32).min(img.height().saturating_sub(1));
    let x_lo = x_start.max(0) as u32;
    let x_hi = ((x_start + w_px) as u32).min(img.width());

    for y in y_lo..=y_hi {
        for x in x_lo..x_hi {
            img.put_pixel(x, y, white);
        }
    }
}

/// 垂直罫線を白で塗りつぶす
fn erase_vertical_line(img: &mut RgbaImage, x_mm: f64, y_mm: f64, height_mm: f64, margin_px: u32) {
    let white = Rgba([255, 255, 255, 255]);
    let x_center = layout::mm_to_px(x_mm).round() as i32;
    let y_start = layout::mm_to_px(y_mm).round() as i32;
    let h_px = layout::mm_to_px(height_mm).round() as i32;

    let x_lo = (x_center - margin_px as i32).max(0) as u32;
    let x_hi = ((x_center + margin_px as i32) as u32).min(img.width().saturating_sub(1));
    let y_lo = y_start.max(0) as u32;
    let y_hi = ((y_start + h_px) as u32).min(img.height());

    for y in y_lo..y_hi {
        for x in x_lo..=x_hi {
            img.put_pixel(x, y, white);
        }
    }
}

/// 期待X位置付近で縦罫線（黒ピクセル）のX座標を探す
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

#[cfg(not(target_arch = "wasm32"))]
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
