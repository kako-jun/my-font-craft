// セル切り出し + チェック欄解析 + 採用判定
use image::{RgbaImage, Rgba};
use crate::layout;
use std::path::Path;

/// チェック欄の状態
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CheckMark {
    Check,  // ✓
    Cross,  // ×
    Empty,  // 空欄
}

/// 1マスの解析結果
#[derive(Debug)]
pub struct SlotResult {
    pub cell_index: usize,   // 0=左, 1=右
    pub is_empty: bool,
    pub black_ratio: f64,
    pub check_mark: CheckMark,
    pub check_density: f64,  // チェック欄の黒ピクセル密度（デバッグ用）
}

/// 1文字（2マス）の採用判定結果
#[derive(Debug)]
pub struct CharResult {
    pub row: usize,
    pub col: usize,
    pub slots: [SlotResult; 2],
    pub adopted: Vec<usize>,     // 採用されたセルインデックス
    pub adoption_reason: String, // 採用理由（デバッグ用）
}

/// 全48文字を処理: セル切り出し → チェック欄解析 → 採用判定
pub fn extract_and_judge(img: &RgbaImage, output_dir: &Path) -> Result<Vec<CharResult>, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("セル出力ディレクトリ作成エラー: {e}"))?;

    // 外枠の枠線(0.5pt≈0.18mm)を避けて内側を切り出す
    // 台形補正の残差+サンプリング太り分も考慮して1.5mm内側 → 12mm×12mm
    let border_margin = 1.0; // mm
    let crop_size = layout::CELL_SIZE - border_margin * 2.0; // 13mm
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;
    let cell_size_px = layout::mm_to_px(layout::CELL_SIZE).round() as u32;
    let check_height_px = layout::mm_to_px(layout::CHECK_HEIGHT).round() as u32;

    let mut results = Vec::new();
    let mut total_adopted = 0usize;
    let mut total_empty = 0usize;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            let mut slots = Vec::new();

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                // 外枠の枠線を除いた内側を切り出し（枠線から1mm内側 = 13mm×13mm）
                let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
                let cell_img = crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px);

                // 空判定: 内側60%領域で黒ピクセル2%未満=空
                let black_ratio = measure_inner_black_ratio(&cell_img, 0.2);
                let is_empty = black_ratio < 0.02;

                // チェック欄切り出し（セル下部の3mm領域、枠線を除く）
                let check_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let check_px_y = layout::mm_to_px(mm_y + layout::CELL_SIZE + border_margin * 0.5).round() as u32;
                let check_w = layout::mm_to_px(layout::CELL_SIZE - border_margin * 2.0).round() as u32;
                let check_h = layout::mm_to_px(layout::CHECK_HEIGHT - border_margin).round() as u32;
                let check_img = crop_region(img, check_px_x, check_px_y, check_w, check_h);

                // チェック欄解析
                let (check_mark, check_density) = analyze_check_mark(&check_img);

                // セル画像を保存
                let filename = format!("R{row:02}C{col:02}_I{cell_idx}.png");
                cell_img.save(output_dir.join(&filename))
                    .map_err(|e| format!("セル保存エラー {filename}: {e}"))?;

                // チェック欄画像も保存
                let check_filename = format!("R{row:02}C{col:02}_I{cell_idx}_check.png");
                check_img.save(output_dir.join(&check_filename))
                    .map_err(|e| format!("チェック欄保存エラー {check_filename}: {e}"))?;

                println!(
                    "  R{row:02}C{col:02}_I{cell_idx}: black={:.1}% {} check={:?}({:.1}%)",
                    black_ratio * 100.0,
                    if is_empty { "空" } else { "非空" },
                    check_mark,
                    check_density * 100.0,
                );

                slots.push(SlotResult {
                    cell_index: cell_idx,
                    is_empty,
                    black_ratio,
                    check_mark,
                    check_density,
                });
            }

            // 採用判定（docs/template-spec.md の採用ルールに従う）
            let (adopted, reason) = judge_adoption(&slots);

            if adopted.is_empty() {
                total_empty += 1;
            } else {
                total_adopted += 1;
            }

            if !adopted.is_empty() || slots.iter().any(|s| !s.is_empty) {
                println!(
                    "  → R{row:02}C{col:02} 採用: {:?} ({})",
                    adopted, reason
                );
            }

            let slots_arr = [slots.remove(0), slots.remove(0)];
            results.push(CharResult {
                row,
                col,
                slots: slots_arr,
                adopted,
                adoption_reason: reason,
            });
        }
    }

    println!("\n  文字サマリー: 採用={total_adopted}, 空={total_empty}, 合計={}", results.len());
    Ok(results)
}

/// 採用判定: docs/template-spec.md の採用ルール
///
/// 1. ×マークのマスは除外
/// 2. ✓マークのマスを採用（複数あれば全てバリエーションとして採用）
/// 3. ✓も×もなければ、一番右の記入済みマスを採用
/// 4. 両方空なら採用なし
fn judge_adoption(slots: &[SlotResult]) -> (Vec<usize>, String) {
    // ×でない非空マスを抽出
    let eligible: Vec<usize> = slots.iter()
        .filter(|s| !s.is_empty && s.check_mark != CheckMark::Cross)
        .map(|s| s.cell_index)
        .collect();

    if eligible.is_empty() {
        return (vec![], "両方空 or 全て×".to_string());
    }

    // ✓付きマスを抽出
    let checked: Vec<usize> = slots.iter()
        .filter(|s| !s.is_empty && s.check_mark == CheckMark::Check)
        .map(|s| s.cell_index)
        .collect();

    if !checked.is_empty() {
        if checked.len() == 2 {
            return (checked, "両方✓ → 2バリエーション".to_string());
        }
        return (checked.clone(), format!("I{}に✓", checked[0]));
    }

    // ✓なし → 一番右の記入済みマスを採用
    let rightmost = *eligible.last().unwrap();
    (vec![rightmost], format!("✓なし → 右(I{rightmost})を採用"))
}

/// チェック欄の解析: 黒ピクセル密度で ✓/×/空欄 を判定
/// Phase 1 (MVP): 密度ベースの簡易判定
fn analyze_check_mark(check_img: &RgbaImage) -> (CheckMark, f64) {
    let w = check_img.width();
    let h = check_img.height();
    if w == 0 || h == 0 {
        return (CheckMark::Empty, 0.0);
    }

    let mut black_count = 0u32;
    let total = w * h;

    for y in 0..h {
        for x in 0..w {
            let p = check_img.get_pixel(x, y);
            let lum = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
            if lum < 128 {
                black_count += 1;
            }
        }
    }

    let density = black_count as f64 / total as f64;

    // 閾値:
    // - 2%未満: 空欄（ノイズや格子線の残骸）
    // - 2%〜15%: ✓（細い線）
    // - 15%以上: ×（太い線、塗りつぶし）
    let mark = if density < 0.02 {
        CheckMark::Empty
    } else if density > 0.15 {
        CheckMark::Cross
    } else {
        CheckMark::Check
    };

    (mark, density)
}

/// 内側領域の黒ピクセル率を計測
fn measure_inner_black_ratio(img: &RgbaImage, margin_ratio: f64) -> f64 {
    let w = img.width();
    let h = img.height();
    let margin_x = (w as f64 * margin_ratio).round() as u32;
    let margin_y = (h as f64 * margin_ratio).round() as u32;

    let mut black_count = 0u32;
    let mut total = 0u32;

    for y in margin_y..(h - margin_y) {
        for x in margin_x..(w - margin_x) {
            total += 1;
            let p = img.get_pixel(x, y);
            let lum = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
            if lum < 128 {
                black_count += 1;
            }
        }
    }

    if total > 0 {
        black_count as f64 / total as f64
    } else {
        0.0
    }
}

/// 領域切り出し
fn crop_region(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut out = RgbaImage::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            let sx = x + dx;
            let sy = y + dy;
            if sx < img.width() && sy < img.height() {
                out.put_pixel(dx, dy, *img.get_pixel(sx, sy));
            } else {
                out.put_pixel(dx, dy, Rgba([255, 255, 255, 255]));
            }
        }
    }
    out
}
