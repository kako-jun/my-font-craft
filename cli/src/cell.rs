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
#[allow(dead_code)] // パイプライン後段で使用予定
pub struct SlotResult {
    pub cell_index: usize,   // 0=左, 1=右
    pub is_empty: bool,
    pub black_ratio: f64,
    pub check_mark: CheckMark,
    pub check_density: f64,  // チェック欄の黒ピクセル密度（デバッグ用）
}

/// 1文字（2マス）の採用判定結果
#[derive(Debug)]
#[allow(dead_code)] // パイプライン後段で使用予定
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
    // 台形補正の残差+サンプリング太り分も考慮して1.0mm内側 → 13mm×13mm
    let border_margin = 1.0; // mm
    let crop_size = layout::CELL_SIZE - border_margin * 2.0; // 13mm
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

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

    let inner_w = w.saturating_sub(margin_x);
    let inner_h = h.saturating_sub(margin_y);
    if margin_x >= inner_w || margin_y >= inner_h {
        return 0.0;
    }

    let mut black_count = 0u32;
    let mut total = 0u32;

    for y in margin_y..inner_h {
        for x in margin_x..inner_w {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── ヘルパー ──

    fn make_slot(cell_index: usize, is_empty: bool, check_mark: CheckMark) -> SlotResult {
        SlotResult {
            cell_index,
            is_empty,
            black_ratio: if is_empty { 0.0 } else { 0.5 },
            check_mark,
            check_density: 0.0,
        }
    }

    fn make_uniform_image(w: u32, h: u32, color: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, color);
            }
        }
        img
    }

    // ── judge_adoption: 14パターン（template-spec.md 採用ルール表） ──

    #[test]
    fn judge_both_filled_both_check() {
        // I0記入✓, I1記入✓ → 両方採用(alt)
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, false, CheckMark::Check),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0, 1]);
    }

    #[test]
    fn judge_both_filled_i0_check_i1_empty_mark() {
        // I0記入✓, I1記入空欄 → I0採用
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_both_filled_i0_check_i1_cross() {
        // I0記入✓, I1記入× → I0採用
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, false, CheckMark::Cross),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_both_filled_i0_empty_mark_i1_check() {
        // I0記入空欄, I1記入✓ → I1採用
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, false, CheckMark::Check),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_both_empty_mark() {
        // I0記入空欄, I1記入空欄 → I1採用（右=後書き優先）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_i0_empty_mark_i1_cross() {
        // I0記入空欄, I1記入× → I0採用（I1は×で除外）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, false, CheckMark::Cross),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_both_filled_i0_cross_i1_empty_mark() {
        // I0記入×, I1記入空欄 → I1採用（I0は×で除外）
        let slots = [
            make_slot(0, false, CheckMark::Cross),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_i0_cross_i1_check() {
        // I0記入×, I1記入✓ → I1採用（I0は×で除外、I1に✓）
        let slots = [
            make_slot(0, false, CheckMark::Cross),
            make_slot(1, false, CheckMark::Check),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_both_cross() {
        // I0記入×, I1記入× → 採用なし
        let slots = [
            make_slot(0, false, CheckMark::Cross),
            make_slot(1, false, CheckMark::Cross),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert!(adopted.is_empty());
    }

    #[test]
    fn judge_i0_filled_i1_empty_no_mark() {
        // I0記入空欄, I1空 → I0採用（唯一の記入済み）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_i0_empty_i1_filled_no_mark() {
        // I0空, I1記入空欄 → I1採用（唯一の記入済み）
        let slots = [
            make_slot(0, true, CheckMark::Empty),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_i0_filled_check_i1_empty() {
        // I0記入✓, I1空 → I0採用
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_i0_filled_cross_i1_empty() {
        // I0記入×, I1空 → 採用なし（唯一の記入だが×）
        let slots = [
            make_slot(0, false, CheckMark::Cross),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert!(adopted.is_empty());
    }

    #[test]
    fn judge_both_empty() {
        // I0空, I1空 → 採用なし（文字未記入）
        let slots = [
            make_slot(0, true, CheckMark::Empty),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert!(adopted.is_empty());
    }

    // ── analyze_check_mark: 閾値境界テスト ──

    #[test]
    fn check_mark_empty_for_white_image() {
        let img = make_uniform_image(50, 20, Rgba([255, 255, 255, 255]));
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Empty);
        assert!(density < 0.02, "density={density} should be < 0.02");
    }

    #[test]
    fn check_mark_check_for_sparse_black() {
        // 密度5%程度 → Check（2%〜15%の範囲）
        let mut img = make_uniform_image(100, 10, Rgba([255, 255, 255, 255]));
        let total = 100 * 10;
        let target_black = (total as f64 * 0.05) as u32;
        let mut count = 0u32;
        'outer: for y in 0..10 {
            for x in 0..100 {
                if count >= target_black { break 'outer; }
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                count += 1;
            }
        }
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Check);
        assert!(density >= 0.02 && density <= 0.15, "density={density}");
    }

    #[test]
    fn check_mark_cross_for_dense_black() {
        // 密度20%程度 → Cross（>15%）
        let mut img = make_uniform_image(100, 10, Rgba([255, 255, 255, 255]));
        let total = 100 * 10;
        let target_black = (total as f64 * 0.20) as u32;
        let mut count = 0u32;
        'outer: for y in 0..10 {
            for x in 0..100 {
                if count >= target_black { break 'outer; }
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                count += 1;
            }
        }
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Cross);
        assert!(density > 0.15, "density={density} should be > 0.15");
    }

    #[test]
    fn check_mark_empty_for_zero_size() {
        let img = RgbaImage::new(0, 0);
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Empty);
        assert_eq!(density, 0.0);
    }

    // ── measure_inner_black_ratio ──

    #[test]
    fn inner_ratio_all_white() {
        let img = make_uniform_image(100, 100, Rgba([255, 255, 255, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn inner_ratio_all_black() {
        let img = make_uniform_image(100, 100, Rgba([0, 0, 0, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert!((ratio - 1.0).abs() < 0.01, "ratio={ratio} should be ~1.0");
    }

    #[test]
    fn inner_ratio_tiny_image_no_underflow() {
        // 1x1画像: margin_ratio=0.2 → margin=0, inner=1
        // margin_x(0) >= inner_w(1) は false なので計算される
        // ただし非常に小さい画像でもパニックしないことが重要
        let img = make_uniform_image(1, 1, Rgba([255, 255, 255, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert!(ratio >= 0.0 && ratio <= 1.0);
    }

    #[test]
    fn inner_ratio_zero_size_image() {
        // 0x0画像 → saturating_sub で安全に0.0を返す
        let img = RgbaImage::new(0, 0);
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert_eq!(ratio, 0.0);
    }
}
