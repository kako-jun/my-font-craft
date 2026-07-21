use crate::vectorizer::PathCommand;
use crate::{cell, layout, marker, perspective, qr, vectorizer};
use image::metadata::Orientation;
/// 画像処理パイプライン（process サブコマンド + WASM用エントリポイント）
use image::{DynamicImage, GrayImage, ImageDecoder, ImageReader, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

use std::io::{BufRead, Cursor, Seek};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// ImageReader からデコードし、Exif の Orientation を適用して RGBA を返す。
///
/// スマホ撮影（特に Android 縦撮り）では画像本体ではなく Exif 側に回転が記録される。
/// image クレートは `open()`/`load_from_memory()` でこれを自動適用しないため、ここで明示的に処理する。
fn decode_oriented_rgba<R: BufRead + Seek>(reader: ImageReader<R>) -> Result<RgbaImage, String> {
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("デコーダ初期化エラー: {e}"))?;
    // Exif が無い・壊れている場合は NoTransforms で続行（ユーザ撮影画像では
    // Exif チャンク欠損が十分にあり得るため、ここではエラーにしたくない）
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut img =
        DynamicImage::from_decoder(decoder).map_err(|e| format!("画像デコードエラー: {e}"))?;
    if orientation != Orientation::NoTransforms {
        log!("  Exif Orientation 適用: {orientation:?}");
        img.apply_orientation(orientation);
    }
    Ok(img.into_rgba8())
}

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
    pub image_data: Vec<u8>, // RGBA生データ（二値化済み: 白背景+黒ストローク）
    pub width: u32,
    pub height: u32,
    /// ベジェパス配列（輪郭単位）。採用セルのみ埋める（空配列の場合あり）
    pub paths: Vec<Vec<PathCommand>>,
    /// セル品質ゲートの結果（#110）。needs_review なら review UI で「要確認」表示する
    pub quality: cell::CellQuality,
}

/// パイプライン全体の処理結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessResult {
    pub page_number: Option<u32>,
    pub total_pages: Option<u32>,
    /// 文字セット選択フラグ（Issue #91, v:3）。'h'/'k'/'a'/'j' を選択順に結合した文字列。
    /// 例: `Some("hk")` → ひらがな+カタカナのみ。`None` は QR からの復元不可。
    pub char_selection: Option<String>,
    /// QR ペイロードの `chars` 配列（リトライ用 PDF のみ）。Issue #96。
    /// 非 None かつ非空のとき、scanner 側はこれを優先し `char_selection` の解決を要求しない。
    pub qr_chars: Option<Vec<String>>,
    pub cells: Vec<ProcessedCell>,
    pub corrected_image: Vec<u8>, // 補正後画像のRGBA
    pub corrected_width: u32,
    pub corrected_height: u32,
    /// シアンサンプルを検出できたか（#111 QA）。false はモノクロ印刷・色褪せ印刷の
    /// 可能性があり、色ベースのシアン除去（内枠・ガイド線の主防御）が
    /// 無効化されているため、scanner 側で UI 警告に昇格する
    pub cyan_sample_detected: bool,
}

// ── DPI算出 ──

/// マーカー間のピクセル距離から実効DPIを算出
fn estimate_dpi(markers: &[marker::DetectedMarker; 4]) -> f64 {
    // markers[0]=TL, markers[1]=TR, markers[2]=BL, markers[3]=BR
    // TL→TR: 198.0mm (水平)
    let h_px =
        ((markers[1].cx - markers[0].cx).powi(2) + (markers[1].cy - markers[0].cy).powi(2)).sqrt();
    let h_dpi = h_px / 198.0 * 25.4;

    // TL→BL: 286.0mm (垂直)
    let v_px =
        ((markers[2].cx - markers[0].cx).powi(2) + (markers[2].cy - markers[0].cy).powi(2)).sqrt();
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
        log!(
            "  ⚠ 解像度が低めです ({dpi:.0} DPI)。処理は続行しますが品質が低下する可能性があります"
        );
    } else {
        log!("  ✓ 解像度: {dpi:.0} DPI (OK)");
    }
    Ok(())
}

// ── CLI用パイプライン（ファイルI/O付き） ──

/// CLI用パイプラインを実行（ファイル読み込み・デバッグ画像保存付き）
#[cfg(not(target_arch = "wasm32"))]
pub fn run_pipeline(image_path: &Path, output_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(output_dir).map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;

    // ステップ1: 画像読み込み
    log!("\n=== ステップ1: 画像読み込み ===");
    let reader = ImageReader::open(image_path)
        .map_err(|e| format!("画像読み込みエラー: {e}"))?
        .with_guessed_format()
        .map_err(|e| format!("画像フォーマット推定エラー: {e}"))?;
    let rgba = decode_oriented_rgba(reader)?;
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

    // ステップ2.6: 中心マーカー事前検出（#132）: 四隅クアッド候補選定のアンカーに使う
    log!("\n=== ステップ2.6: 中心マーカー事前検出 ===");
    let center_hint = marker::detect_center_marker(&binary);
    if center_hint.is_none() {
        log!("  中心マーカー未検出（アンカーなしでクアッド候補を評価）");
    }

    // ステップ3: マーカー検出
    log!("\n=== ステップ3: マーカー検出 ===");
    let markers = marker::detect_markers(&binary, &gray, center_hint.as_ref())?;
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
        let reordered =
            marker::reorder_markers(&markers, tl_index, rotation, rgba.width(), rgba.height());
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
        log!(
            "\n=== ステップ6.5: 補正品質チェック (反復{}) ===",
            iteration + 1
        );
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
                log!(
                    "  反復{}: 残差 {max_residual_mm:.2}mm — 再補正",
                    iteration + 1
                );
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

    // ステップ6.6.5: レンズ歪み補正（9点TPS）
    // 4隅はホモグラフィーで合わせたが、中心がレンズの樽／糸巻き歪みでズレて
    // いるケース（スマホ広角レンズ、紙面までの距離が近い撮影）に対応する。
    log!("\n=== ステップ6.6.5: レンズ歪み補正（9点TPS） ===");
    let (corrected, tps_applied) = apply_lens_tps_correction(corrected)?;
    if tps_applied {
        corrected
            .save(output_dir.join("05d_tps_corrected.png"))
            .map_err(|e| format!("保存エラー: {e}"))?;
        log!("  → 05d_tps_corrected.png 保存完了");
        log!("\n=== ステップ6.6.6: TPS後の中心マーカー残差 ===");
        verify_center_marker(&corrected);
    }

    // ステップ6.7: 罫線直交性チェック＋微小回転補正
    // TPS適用後は9点を厳密に合わせている。中心軸まわりの微小回転を加えると
    // 端部（マーカー位置）がズレてレイアウト前提を壊すため、TPS後はスキップ。
    let corrected = if tps_applied {
        log!("\n=== ステップ6.7: 罫線直交性チェック（TPS適用済みのためスキップ） ===");
        corrected
    } else {
        log!("\n=== ステップ6.7: 罫線直交性チェック ===");
        apply_orthogonality_correction_cli(corrected, output_dir)
    };

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
    let (cyan_removed, _cyan_sample_detected) = remove_cyan(&shadow_corrected);
    cyan_removed
        .save(output_dir.join("08_cyan_removed.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 08_cyan_removed.png 保存完了");

    // ステップ9.3: 紙白正規化 — erase 前に紙の地色を 255 に寄せる
    // （erase 後だと白ストライプが支配的になりヒストグラムが偏る）
    log!("\n=== ステップ9.3: 紙白正規化 ===");
    let paper_normalized = normalize_paper_white(&cyan_removed);
    paper_normalized
        .save(output_dir.join("08a_normalized.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 08a_normalized.png 保存完了");

    // ステップ9.5: 罫線残骸除去
    log!("\n=== ステップ9.5: 罫線残骸除去 ===");
    let grid_removed = erase_grid_lines(&paper_normalized);
    grid_removed
        .save(output_dir.join("08b_grid_removed.png"))
        .map_err(|e| format!("保存エラー: {e}"))?;
    log!("  → 08b_grid_removed.png 保存完了");

    let normalized = grid_removed;

    // ステップ10: セル切り出し + チェック欄解析 + 採用判定
    log!("\n=== ステップ10: セル切り出し + 採用判定 ===");
    let cells_dir = output_dir.join("09_cells");
    cell::extract_and_judge(&normalized, &cells_dir)?;

    log!("\n=== パイプライン完了 ===");
    Ok(())
}

// ── WASM用パイプライン（ファイルI/Oなし） ──

/// WASM用: バイト列から画像を処理して結果を返す
pub fn process_image_bytes(bytes: &[u8]) -> Result<ProcessResult, String> {
    // ステップ1: 画像デコード
    log!("=== ステップ1: 画像デコード ===");
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| format!("画像フォーマット推定エラー: {e}"))?;
    let rgba = decode_oriented_rgba(reader)?;
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

    // ステップ2.6: 中心マーカー事前検出（#132）: 四隅クアッド候補選定のアンカーに使う
    log!("=== ステップ2.6: 中心マーカー事前検出 ===");
    let center_hint = marker::detect_center_marker(&binary);

    // ステップ3: マーカー検出
    log!("=== ステップ3: マーカー検出 ===");
    let markers = marker::detect_markers(&binary, &gray, center_hint.as_ref())?;

    // ステップ4: 向き検出
    log!("=== ステップ4: 向き検出 ===");
    let (tl_index, rotation) = marker::detect_orientation(&binary, &markers)?;

    let (oriented_img, oriented_markers) = if rotation != 0 {
        log!("  画像を{rotation}°回転します");
        let rotated = marker::rotate_image(&rgba, rotation);
        let reordered =
            marker::reorder_markers(&markers, tl_index, rotation, rgba.width(), rgba.height());
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
                log!(
                    "  反復{}: 残差 {max_residual_mm:.2}mm — 再補正",
                    iteration + 1
                );
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

    // ステップ6.6.5: レンズ歪み補正（9点TPS）
    log!("=== レンズ歪み補正（9点TPS） ===");
    let (corrected, tps_applied) = apply_lens_tps_correction(corrected)?;
    if tps_applied {
        verify_center_marker(&corrected);
    }

    // ステップ6.7: 罫線直交性チェック（TPS適用後はスキップ — 端部ズレ防止）
    let corrected = if tps_applied {
        log!("=== 罫線直交性チェック（TPS適用済みのためスキップ） ===");
        corrected
    } else {
        log!("=== 罫線直交性チェック ===");
        apply_orthogonality_correction_wasm(corrected)
    };

    // ステップ7: QR読み取り
    log!("=== QR読み取り ===");
    let qr_info = match read_qr_from_corrected_wasm(&corrected) {
        Ok(data) => {
            log!("  QRデータ: {data}");
            let parsed = parse_qr_payload(&data);
            if parsed.is_empty() {
                let preview: String = data.chars().take(80).collect();
                log!("  ⚠ v:3 JSON として認識できませんでした（p/v/s いずれか欠落または不正）: {preview}");
            }
            parsed
        }
        Err(e) => {
            log!("  QR読み取り失敗（続行）: {e}");
            QrInfo::empty()
        }
    };
    let page_number = qr_info.page;
    let total_pages = qr_info.total;
    let char_selection = qr_info.selection;
    let qr_chars = qr_info.chars;

    // ステップ7.5: ホワイトバランス補正
    log!("=== ホワイトバランス補正 ===");
    let wb_corrected = correct_white_balance(&corrected);

    // ステップ8: 影補正
    log!("=== 影補正 ===");
    let shadow_corrected = correct_shadow(&wb_corrected);

    // ステップ9: シアン除去
    log!("=== シアン除去 ===");
    let (cyan_removed, cyan_sample_detected) = remove_cyan(&shadow_corrected);

    // ステップ9.3: 紙白正規化
    log!("=== 紙白正規化 ===");
    let paper_normalized = normalize_paper_white(&cyan_removed);

    // ステップ9.5: 罫線残骸除去
    log!("=== 罫線残骸除去 ===");
    let normalized = erase_grid_lines(&paper_normalized);

    // ステップ10: セル切り出し + 採用判定
    log!("=== セル切り出し + 採用判定 ===");
    let char_results = cell::extract_and_judge_in_memory(&normalized)?;

    // 結果をProcessResult に変換
    let corrected_width = normalized.width();
    let corrected_height = normalized.height();

    let mut cells = Vec::new();
    for cr in &char_results {
        let char_index = layout::grid_to_char_index(cr.row, cr.col);
        for slot in &cr.slots {
            // 生セルを一度だけ二値化+品質ゲート（#110）し、
            // プレビュー RGBA とベクター化に同じバイナリを使う（結果が必ず一致する）
            let raw_cell =
                cell::extract_cell_image_raw(&normalized, cr.row, cr.col, slot.cell_index);
            let cell_w = raw_cell.width();
            let cell_h = raw_cell.height();
            let (gated_binary, mut quality) = vectorizer::binarize_with_quality(&raw_cell);
            let adopted = cr.adopted.contains(&slot.cell_index);
            // 採用セルは vectorize_adopted_with_review を通す。採用されたのにベクター化が
            // 空（MAX ガード発火 or pre-gate 消失）なら needs_review を立てて #108 の
            // 「黙って欠字」を防ぐ（#112）。
            let paths = if adopted {
                vectorizer::vectorize_adopted_with_review(
                    &gated_binary,
                    cell_w,
                    cell_h,
                    &mut quality,
                )
            } else {
                Vec::new()
            };

            // needs_review はゲート（#110）とベクター化空検知（#112）の双方が立てうる。
            // フラグ済みセルはここで一度だけログする（重複ログを出さない）。
            if quality.needs_review {
                log!(
                    "  ⚠ R{:02}C{:02}_I{}: 品質ゲート要確認 (removed={}, removed_area={:.2}%, kept={}, ink={:.1}%)",
                    cr.row, cr.col, slot.cell_index,
                    quality.removed_components,
                    quality.removed_area_ratio * 100.0,
                    quality.kept_components,
                    quality.ink_ratio * 100.0,
                );
            }

            let binarized = vectorizer::binary_to_rgba(&gated_binary, cell_w, cell_h);
            let image_data = binarized.into_raw();

            cells.push(ProcessedCell {
                row: cr.row,
                col: cr.col,
                char_index,
                is_empty: slot.is_empty,
                adopted,
                cell_index: slot.cell_index,
                image_data,
                width: cell_w,
                height: cell_h,
                paths,
                quality,
            });
        }
    }

    let corrected_image = normalized.into_raw();

    Ok(ProcessResult {
        page_number,
        total_pages,
        char_selection,
        qr_chars,
        cells,
        corrected_image,
        corrected_width,
        corrected_height,
        cyan_sample_detected,
    })
}

/// QR ペイロードから抽出したページ情報と文字セット選択フラグ。
///
/// 3 要素タプルより意図が読みやすいので struct で包む（Issue #91 PR レビュー対応）。
#[derive(Debug, PartialEq)]
struct QrInfo {
    page: Option<u32>,
    total: Option<u32>,
    selection: Option<String>,
    /// リトライ用 PDF の `chars` 配列（Issue #96）。
    /// 各要素は単一文字（`chars().count() == 1`）の `String`。
    chars: Option<Vec<String>>,
}

impl QrInfo {
    fn empty() -> Self {
        Self {
            page: None,
            total: None,
            selection: None,
            chars: None,
        }
    }

    fn is_empty(&self) -> bool {
        self.page.is_none()
            && self.total.is_none()
            && self.selection.is_none()
            && self.chars.is_none()
    }
}

/// QRデータからページ情報・文字セット選択フラグ・文字リストを抽出する（v:3 のみサポート）
///
/// 形式: JSON `{"p":"mfc","v":3,"pg":N,"t":M,"m":2,"s":"<flag>"}`
///       または `{"p":"mfc","v":3,"pg":N,"t":M,"m":2,"chars":["a","b",...]}` (リトライ用 PDF, Issue #96)
/// - `p` が "mfc" かつ `v` が 3 かつ (`s` が非空 OR `chars` が非空配列) のとき各要素を返す。
/// - 上記いずれも満たさない場合は全て `None`（後方互換なし、Issue #91/#96）。
/// - `s` が空文字列 `""` の場合は不正として扱う。
/// - `chars` 配列の要素が単一文字でない（空文字、複数文字、文字列以外）が混在した場合は
///   chars 全体を `None` にする。
fn parse_qr_payload(data: &str) -> QrInfo {
    let trimmed = data.trim();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        let product_ok = v.get("p").and_then(|x| x.as_str()) == Some("mfc");
        let version_ok = v.get("v").and_then(|x| x.as_u64()) == Some(3);
        let selection = v
            .get("s")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // chars 配列の検証: 各要素が「単一文字 (chars().count() == 1) の string」であること。
        // 1 要素でも不正があれば None。空配列も None。
        let chars: Option<Vec<String>> =
            v.get("chars").and_then(|x| x.as_array()).and_then(|arr| {
                if arr.is_empty() {
                    return None;
                }
                let mut out: Vec<String> = Vec::with_capacity(arr.len());
                for item in arr {
                    let s = item.as_str()?;
                    if s.chars().count() != 1 {
                        return None;
                    }
                    out.push(s.to_string());
                }
                Some(out)
            });

        if product_ok && version_ok && (selection.is_some() || chars.is_some()) {
            let page = v
                .get("pg")
                .and_then(|x| x.as_u64())
                .and_then(|n| u32::try_from(n).ok());
            let total = v
                .get("t")
                .and_then(|x| x.as_u64())
                .and_then(|n| u32::try_from(n).ok());
            return QrInfo {
                page,
                total,
                selection,
                chars,
            };
        }
    }

    QrInfo::empty()
}

#[cfg(test)]
mod qr_parse_tests {
    use super::{parse_qr_payload, QrInfo};

    #[test]
    fn v3_json_basic() {
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":2,"m":2,"s":"h"}"#),
            QrInfo {
                page: Some(1),
                total: Some(2),
                selection: Some("h".to_string()),
                chars: None
            },
        );
    }

    #[test]
    fn v3_json_all_sets() {
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":3,"t":10,"m":2,"s":"hkaj"}"#),
            QrInfo {
                page: Some(3),
                total: Some(10),
                selection: Some("hkaj".to_string()),
                chars: None
            },
        );
    }

    #[test]
    fn v3_json_with_whitespace() {
        assert_eq!(
            parse_qr_payload("  {\"p\":\"mfc\",\"v\":3,\"pg\":5,\"t\":7,\"s\":\"hk\"}  "),
            QrInfo {
                page: Some(5),
                total: Some(7),
                selection: Some("hk".to_string()),
                chars: None
            },
        );
    }

    #[test]
    fn v3_missing_s_and_chars_rejected() {
        // s も chars も無ければ拒否（Issue #96 で「s OR chars」に緩和）
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":2,"m":2}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_empty_s_rejected() {
        // TS 側 `flagToSelection('')` が null を返す挙動との対称性のため
        // 空文字列 `""` の `s` も不正として全 None を返す（PR #95 レビュー対応）
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":2,"m":2,"s":""}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_wrong_version_rejected() {
        // v:2 は後方互換なし（Issue #91）
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":2,"pg":1,"t":2,"m":2,"s":"h"}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_wrong_product_rejected() {
        assert_eq!(
            parse_qr_payload(r#"{"p":"other","v":3,"pg":1,"t":2,"s":"h"}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_overflow_u32() {
        // u32::MAX 超の値は try_from で弾かれて None になる
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":4294967296,"t":2,"s":"h"}"#),
            QrInfo {
                page: None,
                total: Some(2),
                selection: Some("h".to_string()),
                chars: None
            },
        );
    }

    // ── Issue #96: chars 配列対応（リトライ用 PDF） ──

    #[test]
    fn v3_chars_only_accepted() {
        // chars だけ・s なしでも受理される（リトライ用 PDF の最小ケース）
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":["あ","い","う"]}"#),
            QrInfo {
                page: Some(1),
                total: Some(1),
                selection: None,
                chars: Some(vec!["あ".to_string(), "い".to_string(), "う".to_string()]),
            },
        );
    }

    #[test]
    fn v3_chars_and_s_both_accepted() {
        // chars と s 両方ありなら両方取れる
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":2,"t":3,"m":2,"s":"h","chars":["a","b"]}"#),
            QrInfo {
                page: Some(2),
                total: Some(3),
                selection: Some("h".to_string()),
                chars: Some(vec!["a".to_string(), "b".to_string()]),
            },
        );
    }

    #[test]
    fn v3_chars_with_empty_string_element_rejects_chars() {
        // 不正要素混在で chars 全体が None。s も無いので全体 empty。
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":["a","",""]}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_chars_with_multichar_element_rejects_chars() {
        // 2 文字要素混在で chars 全体が None
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":["a","bc"]}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_chars_with_non_string_element_rejects_chars() {
        // 数値要素混在で chars 全体が None
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":["a",1]}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_chars_empty_array_rejects_chars() {
        // 空配列は None。s もないので empty
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":[]}"#),
            QrInfo::empty(),
        );
    }

    #[test]
    fn v3_chars_empty_array_but_s_present_keeps_s() {
        // chars が空配列でも s があるなら受理（chars だけ None）
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"s":"h","chars":[]}"#),
            QrInfo {
                page: Some(1),
                total: Some(1),
                selection: Some("h".to_string()),
                chars: None
            },
        );
    }

    #[test]
    fn v3_chars_with_surrogate_pair_accepted() {
        // サロゲートペア (U+1F600 等) は Rust の `chars()` でも 1 要素扱いなので受理される
        assert_eq!(
            parse_qr_payload(r#"{"p":"mfc","v":3,"pg":1,"t":1,"m":2,"chars":["😀"]}"#),
            QrInfo {
                page: Some(1),
                total: Some(1),
                selection: None,
                chars: Some(vec!["😀".to_string()]),
            },
        );
    }

    #[test]
    fn invalid_returns_none() {
        assert_eq!(parse_qr_payload("garbage"), QrInfo::empty());
    }

    #[test]
    fn empty_string_returns_none() {
        assert_eq!(parse_qr_payload(""), QrInfo::empty());
    }

    #[test]
    fn v1_plain_text_rejected() {
        // v:1 プレーンテキストは後方互換なしで拒否
        assert_eq!(parse_qr_payload("mfc:1/3"), QrInfo::empty());
    }
}

// ── 共通処理関数（CLI/WASM両方で使用） ──

/// 補正品質チェック（画像保存なし版 — WASM用）
fn verify_correction_quality_wasm(
    corrected: &RgbaImage,
) -> Option<(f64, [marker::DetectedMarker; 4])> {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);
    let center_hint = marker::detect_center_marker(&binary);

    match marker::detect_markers(&binary, &gray, center_hint.as_ref()) {
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
fn verify_correction_quality_cli(
    corrected: &RgbaImage,
    output_dir: &Path,
) -> Option<(f64, [marker::DetectedMarker; 4])> {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);
    let center_hint = marker::detect_center_marker(&binary);

    match marker::detect_markers(&binary, &gray, center_hint.as_ref()) {
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

                let status = if err_mm < 0.5 {
                    "OK"
                } else if err_mm < 1.0 {
                    "注意"
                } else {
                    "要改善"
                };

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

/// TPS を発動する中心残差の下限（mm）。これ以下ならホモグラフィーで十分。
const TPS_MIN_RESIDUAL_MM: f64 = 1.0;

/// 警告を出し始める中心残差（mm）。これを超えたら TPS は適用するが、品質が
/// 低下している可能性をログに残す。スマホ広角撮影では 5〜10mm が現実的に発生する。
const LENS_DISTORTION_WARN_MM: f64 = 5.0;

/// 重度レンズ歪みのしきい値（mm）。
/// ホモグラフィー後の中心残差がこれを超える画像は、9点TPSでは境界の樽型歪みを
/// 吸収しきれず、紙面上部〜中段のセル切り出しが崩れる（Issue #88 参照）。
/// 「撮影距離を見直してください」をユーザーに促す。
/// Issue #92: 5mm → 10mm に引き上げ（スマホ広角での実用性を優先）。
const SEVERE_LENS_DISTORTION_MM: f64 = 10.0;

/// 中心マーカーを再検出し、残差が閾値超なら9点TPSで再ワープする（CLI/WASM共通）。
///
/// 戻り値: Ok((補正後画像, TPSを実際に適用したか))
/// 重度歪み（> SEVERE_LENS_DISTORTION_MM）の場合は Err を返して上位で中断させる。
///
/// 4隅マーカーはホモグラフィーで合わせ切れているが、中心がズレているケース
/// （= カメラのレンズ歪み）だけを対象にする。ここでTPSを効かせすぎると
/// むしろノイズを拾うので、1.0mm 以下なら何もしない。
fn apply_lens_tps_correction(corrected: RgbaImage) -> Result<(RgbaImage, bool), String> {
    let gray = DynamicImage::ImageRgba8(corrected.clone()).into_luma8();
    let threshold = marker::otsu_threshold(&gray);
    let binary = marker::binarize(&gray, threshold);

    let Some(center_detected) = marker::detect_center_marker(&binary) else {
        log!("  中心マーカー未検出 — TPS補正をスキップ");
        return Ok((corrected, false));
    };

    let (exp_cx_mm, exp_cy_mm) = layout::center_marker_center();
    let exp_cx = layout::mm_to_px(exp_cx_mm);
    let exp_cy = layout::mm_to_px(exp_cy_mm);
    let dcx = center_detected.cx - exp_cx;
    let dcy = center_detected.cy - exp_cy;
    let err_mm = (dcx * dcx + dcy * dcy).sqrt() / layout::mm_to_px(1.0);

    // ホモグラフィーで十分ならTPSは入れない（画質安定優先）
    if err_mm <= TPS_MIN_RESIDUAL_MM {
        log!("  中心残差 {err_mm:.2}mm — TPS補正は不要");
        return Ok((corrected, false));
    }

    // 重度歪み: 撮り直しを要請する（TPSを走らせてもセル抽出が崩れるため）
    if err_mm > SEVERE_LENS_DISTORTION_MM {
        return Err(format!(
            "レンズ歪みが大きすぎます（中心残差 {err_mm:.1}mm、許容 {:.1}mm以下）。\
             もう一歩離れて撮り直してください。",
            SEVERE_LENS_DISTORTION_MM
        ));
    }

    // 中程度の歪み: TPS を適用するが品質低下の可能性を警告
    if err_mm > LENS_DISTORTION_WARN_MM {
        log!(
            "  ⚠ 中心残差 {err_mm:.2}mm は大きめです — TPS を適用しますが、\
             紙端のセル切り出し精度が落ちる可能性があります（結果を確認してください）"
        );
    }

    let corners = match marker::detect_markers(&binary, &gray, Some(&center_detected)) {
        Ok(c) => c,
        Err(e) => {
            log!("  ⚠ 4隅マーカー再検出失敗 ({e}) — TPS補正をスキップ");
            return Ok((corrected, false));
        }
    };

    let marker_defs = [
        layout::MARKER_TL,
        layout::MARKER_TR,
        layout::MARKER_BL,
        layout::MARKER_BR,
    ];

    // 4隅 → ホモグラフィー後の検出位置と期待位置
    let mut corner_src = [(0.0, 0.0); 4];
    let mut corner_dst = [(0.0, 0.0); 4];
    for i in 0..4 {
        corner_src[i] = (corners[i].cx, corners[i].cy);
        let (cx, cy) = layout::marker_center(&marker_defs[i]);
        corner_dst[i] = (layout::mm_to_px(cx), layout::mm_to_px(cy));
    }

    // 4辺中点を制御点として追加。
    // ホモグラフィー後は紙面の辺は直線として保たれているはずなので、
    // src（検出側）も dst（期待側）も「対応する2隅の中点」として固定する。
    // これで境界部はホモグラフィー精度のまま固定され、TPSの引き戻しは
    // 紙面内側にだけ作用する。
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
    let src_pts: Vec<(f64, f64)> = vec![
        corner_src[0],
        corner_src[1],
        corner_src[2],
        corner_src[3],
        mid(corner_src[0], corner_src[1]), // top
        mid(corner_src[2], corner_src[3]), // bottom
        mid(corner_src[0], corner_src[2]), // left
        mid(corner_src[1], corner_src[3]), // right
        (center_detected.cx, center_detected.cy),
    ];
    let dst_pts: Vec<(f64, f64)> = vec![
        corner_dst[0],
        corner_dst[1],
        corner_dst[2],
        corner_dst[3],
        mid(corner_dst[0], corner_dst[1]),
        mid(corner_dst[2], corner_dst[3]),
        mid(corner_dst[0], corner_dst[2]),
        mid(corner_dst[1], corner_dst[3]),
        (exp_cx, exp_cy),
    ];

    log!("  中心残差 {err_mm:.2}mm — 9点TPS（4隅+4辺中点+中心）でレンズ歪み補正を適用");
    let warped = perspective::tps_warp(&corrected, &src_pts, &dst_pts);
    Ok((warped, true))
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

            let status = if err_mm < 0.5 {
                "OK"
            } else if err_mm < 1.0 {
                "注意"
            } else if err_mm < 3.0 {
                "要改善"
            } else {
                "レンズ歪みの可能性"
            };

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
            log!(
                "  ⚠ 残留角度 {:.2}° が大きすぎます — 台形補正が失敗している可能性があります",
                angle.abs()
            );
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
            log!(
                "  ⚠ 残留角度 {:.2}° が大きすぎます — 台形補正が失敗している可能性があります",
                angle.abs()
            );
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
    let y_bottom =
        layout::mm_to_px(layout::BODY_START_Y + 10.0 * layout::ROW_HEIGHT + 2.0).round() as u32;

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
            log!("  縦罫線 x={col_x_mm:.0}mm: top_x={tx} bottom_x={bx} 角度={angle_deg:.3}°");
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

        log!("  バーステップ[{i}]: 期待={expected:.0} 左実測={left_avg:.1} 右実測={right_avg:.1}");
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
        (
            sum_r / count as f64,
            sum_g / count as f64,
            sum_b / count as f64,
        )
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
    if coeff_r < 0.5
        || coeff_r > 2.0
        || coeff_g < 0.5
        || coeff_g > 2.0
        || coeff_b < 0.5
        || coeff_b > 2.0
    {
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

// シアン除去のしきい値（緩め: 薄シアン対応）。テストで境界値を固定するため module スコープ
const CYAN_SAMPLE_SCORE_MIN: f64 = 1.5;
const CYAN_SCORE_THRESHOLD: i32 = 3;
const MIN_BRIGHTNESS: i32 = 140;

/// シアン除去: "cyan signature" (G,B が R より高い) を閾値判定で白化
/// 固定 RGB 距離より、薄いシアン縁まで拾えて黒ストロークを保護しやすい。
/// - cyan_score = min(G, B) - R: 純シアンなら ~50、薄い縁でも 5〜20。黒ペンは 0 前後
/// - 暗いピクセル(輝度低)は手書きとしてゲート除外
///
/// 戻り値: (処理後画像, シアンサンプルを検出できたか)。
/// サンプル未検出（= モノクロ印刷・色褪せ印刷の可能性）は第2要素 false。
/// モノクロ印刷では内枠・ガイド線が薄グレーで印字されて色ベースの除去（L1）が
/// 無力化し、濃い印字だと第2防御 erase_grid_lines の輝度150保護も同時に素通りする。
/// そのため未検出は WASM 出力 → scanner の UI 警告に昇格する（#111 QA）
fn remove_cyan(img: &RgbaImage) -> (RgbaImage, bool) {
    let sample_x = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
    let sample_y = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
    let sample_size = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;
    let (cyan_r, cyan_g, cyan_b) =
        sample_region_rgb(img, sample_x, sample_y, sample_size, sample_size);
    log!("  シアンサンプル平均色: R={cyan_r:.1} G={cyan_g:.1} B={cyan_b:.1}");

    // 検出できなくても erase_grid_lines (inner_margin=5px)
    // が layout 既知で内枠を白塗りするので即座に致命的ではない
    let sample_score = cyan_g.min(cyan_b) - cyan_r;
    if sample_score < CYAN_SAMPLE_SCORE_MIN {
        log!("  ⚠ シアンサンプルに有意な cyan 成分なし (score={sample_score:.1}) — スキップ");
        return (img.clone(), false);
    }

    let mut out = img.clone();
    let mut removed_count = 0u64;

    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            let r = p[0] as i32;
            let g = p[1] as i32;
            let b = p[2] as i32;
            let avg = (r + g + b) / 3;
            if avg < MIN_BRIGHTNESS {
                continue;
            }
            let cyan_score = g.min(b) - r;
            if cyan_score >= CYAN_SCORE_THRESHOLD {
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

    (out, true)
}

/// 紙白正規化: 紙色（ヒストグラム最頻値）を 255 にスケールし、紙の地色を純白に寄せる
/// shadow_correct 後でも紙は灰色（~230）のまま残るため、
/// Sauvola がその濃淡をノイズとして拾う。
/// 輝度 100 以上の最頻値を「紙色」とみなして線形ストレッチする
/// （100 未満は手書きインク、100 以上は紙＋シアン残骸＋ノイズ）
fn normalize_paper_white(img: &RgbaImage) -> RgbaImage {
    const PAPER_LUMINANCE_FLOOR: usize = 100;

    let mut hist = [0u64; 256];
    for p in img.pixels() {
        let lum = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000;
        hist[lum.min(255) as usize] += 1;
    }
    let mut mode_lum = 230u32;
    let mut mode_count = 0u64;
    for (i, &c) in hist.iter().enumerate().skip(PAPER_LUMINANCE_FLOOR) {
        if c > mode_count {
            mode_count = c;
            mode_lum = i as u32;
        }
    }
    if mode_lum < 180 {
        log!("  ⚠ 紙色モードが低すぎ ({mode_lum}) — 正規化をスキップ");
        return img.clone();
    }
    if mode_lum >= 250 {
        log!("  紙はすでに十分白 (mode={mode_lum}) — スキップ");
        return img.clone();
    }
    log!("  紙色モード={mode_lum} → 255 にストレッチ");
    let scale = 255.0 / mode_lum as f64;
    let mut out = img.clone();
    for y in 0..img.height() {
        for x in 0..img.width() {
            let p = img.get_pixel(x, y);
            let r = ((p[0] as f64) * scale).min(255.0) as u8;
            let g = ((p[1] as f64) * scale).min(255.0) as u8;
            let b = ((p[2] as f64) * scale).min(255.0) as u8;
            out.put_pixel(x, y, Rgba([r, g, b, p[3]]));
        }
    }
    out
}

/// 罫線残骸除去: レイアウト定数から罫線位置を算出し、±2px を白で塗りつぶす
fn erase_grid_lines(img: &RgbaImage) -> RgbaImage {
    let mut out = img.clone();
    // #34: 2px では TPS 後の残差ズレ（数px）で外枠の消し残しが発生するケースがあった。
    // 手書き文字は内枠（cyan）より内側に集中する前提で、外枠の消し幅は 6px まで拡張する。
    // 6px = 0.5mm@300dpi なので手書きへの影響は実質ゼロ。
    let line_margin = 6u32;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                erase_horizontal_line(&mut out, mm_x, mm_y, layout::CELL_SIZE, line_margin);
                erase_horizontal_line(
                    &mut out,
                    mm_x,
                    mm_y + layout::CELL_SIZE,
                    layout::CELL_SIZE,
                    line_margin,
                );
                erase_vertical_line(&mut out, mm_x, mm_y, layout::CELL_SIZE, line_margin);
                erase_vertical_line(
                    &mut out,
                    mm_x + layout::CELL_SIZE,
                    mm_y,
                    layout::CELL_SIZE,
                    line_margin,
                );

                // 内枠（シアン）は台形補正の残差＋縁のぼかしで幅が出やすいので、
                // 外枠より広いマージンで塗り潰す。手書きは内枠線の内側に集中する前提
                let inner_margin = 5u32;
                let inner_offset = (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0;
                let ix = mm_x + inner_offset;
                let iy = mm_y + inner_offset;
                erase_horizontal_line(&mut out, ix, iy, layout::INNER_SIZE, inner_margin);
                erase_horizontal_line(
                    &mut out,
                    ix,
                    iy + layout::INNER_SIZE,
                    layout::INNER_SIZE,
                    inner_margin,
                );
                erase_vertical_line(&mut out, ix, iy, layout::INNER_SIZE, inner_margin);
                erase_vertical_line(
                    &mut out,
                    ix + layout::INNER_SIZE,
                    iy,
                    layout::INNER_SIZE,
                    inner_margin,
                );

                // ベースライン/センターガイド線（#111）。
                // 除去の主防御はシアン除去。ここは layout 既知の第2防御で、
                // 輝度150未満の手書きストロークは is_overpaintable が保護する。
                // ガイド線はセル中央付近を横切るため、シアン除去も本白塗りも
                // 抜けた場合はセル品質ゲート（#110）の境界接触除去には掛からない
                // （境界帯に触れない）— ゲートはあくまで保険であり主防御ではない
                // 4px は帯の中で最小: ガイドは手書きの最密集地帯（書く領域の中央）を
                // 通るため外枠 6px・内枠 5px より狭くして誤白塗りの面を減らす。
                // TPS 残差が 4px を超えるケースはシアン除去（色ベース・位置非依存）が主防御
                let guide_margin = 4u32;
                erase_horizontal_line(
                    &mut out,
                    ix,
                    iy + layout::INNER_SIZE - layout::GUIDE_BASELINE_OFFSET_MM,
                    layout::INNER_SIZE,
                    guide_margin,
                );
                erase_vertical_line(
                    &mut out,
                    mm_x + layout::CELL_SIZE / 2.0,
                    iy,
                    layout::INNER_SIZE,
                    guide_margin,
                );

                let check_y = mm_y + layout::CELL_SIZE;
                erase_horizontal_line(
                    &mut out,
                    mm_x,
                    check_y + layout::CHECK_HEIGHT,
                    layout::CELL_SIZE,
                    line_margin,
                );
                erase_vertical_line(&mut out, mm_x, check_y, layout::CHECK_HEIGHT, line_margin);
                erase_vertical_line(
                    &mut out,
                    mm_x + layout::CELL_SIZE,
                    check_y,
                    layout::CHECK_HEIGHT,
                    line_margin,
                );
            }

            let (sx, sy) = layout::get_sample_position(row, col);
            erase_horizontal_line(&mut out, sx, sy, layout::SAMPLE_WIDTH, line_margin);
            erase_horizontal_line(
                &mut out,
                sx,
                sy + layout::CELL_SIZE,
                layout::SAMPLE_WIDTH,
                line_margin,
            );
            erase_vertical_line(&mut out, sx, sy, layout::CELL_SIZE, line_margin);
            erase_vertical_line(
                &mut out,
                sx + layout::SAMPLE_WIDTH,
                sy,
                layout::CELL_SIZE,
                line_margin,
            );
        }
    }

    log!("  罫線残骸除去完了");
    out
}

/// 水平罫線を白で塗りつぶす
/// 白塗り対象ピクセルか判定: 暗い（= 黒インク）は保護する
/// シアン枠は彩度が高くても輝度も高めなので、この閾値で除去できる
/// 注意: erase_grid_lines は normalize_paper_white の後に走るため、
/// 紙色は ≈255 まで持ち上がっている。黒インクは ~30→~40 程度なので
/// 閾値 150 は十分な安全マージン。薄鉛筆等は normalize で 200+ に
/// 寄るため白塗りされる可能性があるが、そもそも Sauvola で拾えない濃度なので実害なし
fn is_overpaintable(p: &Rgba<u8>) -> bool {
    let lum = p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114;
    lum >= 150 * 1000 // 輝度 150/255 以上なら塗ってよい
}

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
            let p = *img.get_pixel(x, y);
            if is_overpaintable(&p) {
                img.put_pixel(x, y, white);
            }
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
            let p = *img.get_pixel(x, y);
            if is_overpaintable(&p) {
                img.put_pixel(x, y, white);
            }
        }
    }
}

/// 期待X位置付近で縦罫線（黒ピクセル）のX座標を探す
fn find_grid_line_x(binary: &GrayImage, expected_x: i32, y: u32, search_range: i32) -> Option<i32> {
    let mut best_x = None;
    let mut min_dist = search_range + 1;

    for dy in -2i32..=2 {
        let sy = (y as i32 + dy).max(0) as u32;
        if sy >= binary.height() {
            continue;
        }

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

// ── 画像処理関数のテスト（#111 QA） ──

#[cfg(all(test, not(target_arch = "wasm32")))]
mod image_ops_tests {
    use super::*;

    fn white_page() -> RgbaImage {
        RgbaImage::from_pixel(
            layout::image_width(),
            layout::image_height(),
            Rgba([255, 255, 255, 255]),
        )
    }

    /// セル(0,0,I0) のガイド線公称位置（erase_grid_lines と同じ丸めで算出）
    /// 戻り値: (baseline_y, center_x, inner_left_x, inner_top_y)
    fn cell_guide_coords() -> (u32, u32, u32, u32) {
        let (mm_x, mm_y) = layout::get_cell_position(0, 0, 0);
        let inner_offset = (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0;
        let ix = mm_x + inner_offset;
        let iy = mm_y + inner_offset;
        let baseline_y =
            layout::mm_to_px(iy + layout::INNER_SIZE - layout::GUIDE_BASELINE_OFFSET_MM).round()
                as u32;
        let center_x = layout::mm_to_px(mm_x + layout::CELL_SIZE / 2.0).round() as u32;
        let inner_left_x = layout::mm_to_px(ix).round() as u32;
        let inner_top_y = layout::mm_to_px(iy).round() as u32;
        (baseline_y, center_x, inner_left_x, inner_top_y)
    }

    const WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

    #[test]
    fn erase_grid_lines_whitens_baseline_guide_within_margin_only() {
        // ガイド白塗り帯の境界固定: 公称位置と ±4px は白化、±5px は残存（帯の外）
        let (baseline_y, center_x, inner_left_x, _) = cell_guide_coords();
        let gray = Rgba([200, 200, 200, 255]);
        // 検査セグメントは縦帯（センターガイド ±4px・内枠左 ±5px）を避けた x 範囲
        let seg_x0 = inner_left_x + 8;
        let seg_x1 = center_x - 8;
        let mut img = white_page();
        for x in seg_x0..seg_x1 {
            img.put_pixel(x, baseline_y, gray); // 公称位置
            img.put_pixel(x, baseline_y + 4, gray); // 帯の内側端（+側、inclusive）
            img.put_pixel(x, baseline_y + 5, gray); // 帯の外（+側）
            img.put_pixel(x, baseline_y - 4, gray); // 帯の内側端（-側、inclusive）
            img.put_pixel(x, baseline_y - 5, gray); // 帯の外（-側）
        }
        let out = erase_grid_lines(&img);
        for x in seg_x0..seg_x1 {
            assert_eq!(
                *out.get_pixel(x, baseline_y),
                WHITE,
                "公称位置 x={x} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(x, baseline_y + 4),
                WHITE,
                "+4px x={x} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(x, baseline_y + 5),
                gray,
                "+5px x={x} は帯の外で残存すべき"
            );
            assert_eq!(
                *out.get_pixel(x, baseline_y - 4),
                WHITE,
                "-4px x={x} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(x, baseline_y - 5),
                gray,
                "-5px x={x} は帯の外で残存すべき"
            );
        }
    }

    #[test]
    fn erase_grid_lines_whitens_center_guide_within_margin_only() {
        let (baseline_y, center_x, _, inner_top_y) = cell_guide_coords();
        let gray = Rgba([200, 200, 200, 255]);
        // 検査セグメントは横帯（内枠上 ±5px・ベースライン ±4px）を避けた y 範囲
        let seg_y0 = inner_top_y + 8;
        let seg_y1 = baseline_y - 8;
        let mut img = white_page();
        for y in seg_y0..seg_y1 {
            img.put_pixel(center_x, y, gray);
            img.put_pixel(center_x + 4, y, gray);
            img.put_pixel(center_x + 5, y, gray);
            img.put_pixel(center_x - 4, y, gray);
            img.put_pixel(center_x - 5, y, gray);
        }
        let out = erase_grid_lines(&img);
        for y in seg_y0..seg_y1 {
            assert_eq!(
                *out.get_pixel(center_x, y),
                WHITE,
                "公称位置 y={y} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(center_x + 4, y),
                WHITE,
                "+4px y={y} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(center_x + 5, y),
                gray,
                "+5px y={y} は帯の外で残存すべき"
            );
            assert_eq!(
                *out.get_pixel(center_x - 4, y),
                WHITE,
                "-4px y={y} は白化されるべき"
            );
            assert_eq!(
                *out.get_pixel(center_x - 5, y),
                gray,
                "-5px y={y} は帯の外で残存すべき"
            );
        }
    }

    #[test]
    fn erase_grid_lines_luminance_boundary_at_150() {
        // ガイド帯を跨ぐストローク保護の ±1 境界: 輝度149 は保護、150 ちょうどは塗られる
        // （is_overpaintable: lum >= 150*1000 なら塗ってよい）
        let (baseline_y, center_x, inner_left_x, _) = cell_guide_coords();
        let dark149 = Rgba([149, 149, 149, 255]); // lum 149,000 < 150,000 → 保護
        let dark150 = Rgba([150, 150, 150, 255]); // lum 150,000 >= 150,000 → 塗られる
        let x149 = inner_left_x + 10;
        let x150 = inner_left_x + 12;
        assert!(
            x150 < center_x - 4,
            "検査画素がセンターガイド帯に入らないこと"
        );
        let mut img = white_page();
        img.put_pixel(x149, baseline_y, dark149);
        img.put_pixel(x150, baseline_y, dark150);
        let out = erase_grid_lines(&img);
        assert_eq!(
            *out.get_pixel(x149, baseline_y),
            dark149,
            "輝度149 は手書きとして保護"
        );
        assert_eq!(
            *out.get_pixel(x150, baseline_y),
            WHITE,
            "輝度150 は白塗り対象"
        );
    }

    #[test]
    fn remove_cyan_score_and_brightness_thresholds() {
        // cyan_score 境界（>=3 で除去）と輝度ゲート境界（avg>=140 で処理対象）の固定
        let mut img = white_page();
        // シアンサンプル領域を有効な薄シアンで塗る（sample_score = 25 >= 1.5）
        let sx = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
        let sy = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
        let ss = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;
        for y in sy..sy + ss {
            for x in sx..sx + ss {
                img.put_pixel(x, y, Rgba([230, 255, 255, 255]));
            }
        }
        // 検査画素（本文領域内の任意位置）
        img.put_pixel(1000, 1000, Rgba([200, 203, 203, 255])); // score=3, avg=202 → 除去
        img.put_pixel(1010, 1000, Rgba([200, 202, 202, 255])); // score=2 → 残存
        img.put_pixel(1020, 1000, Rgba([135, 143, 143, 255])); // avg=140（=境界）, score=8 → 除去
        img.put_pixel(1030, 1000, Rgba([134, 142, 142, 255])); // avg=139 → 暗色ゲートでスキップ

        let (out, detected) = remove_cyan(&img);
        assert!(detected, "有効なサンプルがあるので検出される");
        assert_eq!(*out.get_pixel(1000, 1000), WHITE, "cyan_score=3 は除去");
        assert_eq!(
            *out.get_pixel(1010, 1000),
            Rgba([200, 202, 202, 255]),
            "cyan_score=2 は残存"
        );
        assert_eq!(
            *out.get_pixel(1020, 1000),
            WHITE,
            "avg=140 は処理対象（境界含む）"
        );
        assert_eq!(
            *out.get_pixel(1030, 1000),
            Rgba([134, 142, 142, 255]),
            "avg=139 は暗色（手書き）としてスキップ"
        );
    }

    #[test]
    fn remove_cyan_skips_all_when_sample_score_below_threshold() {
        // sample_score < 1.5（シアンサンプルが白 = モノクロ印刷相当）なら
        // 画像中に強いシアンがあっても全スキップし、検出フラグ false を返す
        let mut img = white_page();
        let strong_cyan = Rgba([180, 255, 255, 255]); // score=75
        img.put_pixel(1000, 1000, strong_cyan);
        let (out, detected) = remove_cyan(&img);
        assert!(!detected, "サンプル未検出フラグが返るべき");
        assert_eq!(
            *out.get_pixel(1000, 1000),
            strong_cyan,
            "全スキップで残存すべき"
        );
    }

    #[test]
    fn layout_defense_alone_erases_guides_and_keeps_cells_empty() {
        // ネイティブ統合テスト（#111 QA）: シアンサンプルを白潰しした合成テンプレート
        // （= L1 色ベース除去が無効化された状況）でも、L2 erase_grid_lines だけで
        // 内枠・ガイド線が消え、全マスが空のまま判定されること
        let path = std::env::temp_dir().join("mfc-test-l2-defense-template.png");
        crate::template::generate_template(&path, false).expect("テンプレート生成");
        let mut img = image::open(&path).expect("テンプレート読込").into_rgba8();
        let _ = std::fs::remove_file(&path);

        // シアンサンプル領域を白潰し（モノクロ印刷でシアン成分が失われた状況の代理）
        let sx = layout::mm_to_px(layout::CYAN_SAMPLE_X).round() as u32;
        let sy = layout::mm_to_px(layout::CYAN_SAMPLE_Y).round() as u32;
        let ss = layout::mm_to_px(layout::CYAN_SAMPLE_SIZE).round() as u32;
        for y in sy..sy + ss {
            for x in sx..sx + ss {
                img.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }

        let (after_cyan, detected) = remove_cyan(&img);
        assert!(!detected, "サンプル白潰しで L1 は無効化されるはず");
        let normalized = normalize_paper_white(&after_cyan);
        let erased = erase_grid_lines(&normalized);

        // ガイド線の公称位置が白化されている（セル(0,0,I0) の帯回避セグメントで確認）
        let (baseline_y, center_x, inner_left_x, inner_top_y) = cell_guide_coords();
        for x in (inner_left_x + 8)..(center_x - 8) {
            assert_eq!(
                *erased.get_pixel(x, baseline_y),
                WHITE,
                "ベースラインガイド x={x} が L2 で消えるべき"
            );
        }
        for y in (inner_top_y + 8)..(baseline_y - 8) {
            assert_eq!(
                *erased.get_pixel(center_x, y),
                WHITE,
                "センターガイド y={y} が L2 で消えるべき"
            );
        }

        // 全47文字 × 2マスが空のまま（ガイド線・内枠がインクとして誤検出されない）
        let results = cell::extract_and_judge_in_memory(&erased).expect("セル判定");
        for cr in &results {
            for slot in &cr.slots {
                assert!(
                    slot.is_empty,
                    "R{:02}C{:02}_I{} が非空判定（black={:.2}%）: ガイド/枠がインク扱いされている",
                    cr.row,
                    cr.col,
                    slot.cell_index,
                    slot.black_ratio * 100.0
                );
            }
        }
    }
}
