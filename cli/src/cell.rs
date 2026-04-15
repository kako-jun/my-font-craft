// セル切り出し + チェック欄解析 + 採用判定
use image::{RgbaImage, Rgba};
use serde::{Serialize, Deserialize};
use crate::{log, layout};

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// チェック欄の状態
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CheckMark {
    Check,  // ✓
    Cross,  // ×
    Empty,  // 空欄
}

/// 1マスの解析結果
#[derive(Debug, Serialize, Deserialize)]
pub struct SlotResult {
    pub cell_index: usize,   // 0=左, 1=右
    pub is_empty: bool,
    pub black_ratio: f64,
    pub check_mark: CheckMark,
    pub check_density: f64,  // チェック欄の黒ピクセル密度（デバッグ用）
}

/// 1文字（2マス）の採用判定結果
#[derive(Debug, Serialize, Deserialize)]
pub struct CharResult {
    pub row: usize,
    pub col: usize,
    pub slots: [SlotResult; 2],
    pub adopted: Vec<usize>,     // 採用されたセルインデックス
    pub adoption_reason: String, // 採用理由（デバッグ用）
}

/// CLI用: 全48文字を処理してファイルに保存
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_and_judge(img: &RgbaImage, output_dir: &Path) -> Result<Vec<CharResult>, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("セル出力ディレクトリ作成エラー: {e}"))?;

    let border_margin = 1.0;
    let crop_size = layout::CELL_SIZE - border_margin * 2.0;
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

    let mut results = Vec::new();
    let mut total_adopted = 0usize;
    let mut total_empty = 0usize;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }
            let mut slots = Vec::new();

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
                let cell_img = crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px);

                let black_ratio = measure_inner_black_ratio(&cell_img, 0.2);
                let is_empty = black_ratio < 0.02;

                let check_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let check_px_y = layout::mm_to_px(mm_y + layout::CELL_SIZE + border_margin * 0.5).round() as u32;
                let check_w = layout::mm_to_px(layout::CELL_SIZE - border_margin * 2.0).round() as u32;
                let check_h = layout::mm_to_px(layout::CHECK_HEIGHT - border_margin).round() as u32;
                let check_img = crop_region(img, check_px_x, check_px_y, check_w, check_h);

                let (check_mark, check_density) = analyze_check_mark(&check_img);

                let filename = format!("R{row:02}C{col:02}_I{cell_idx}.png");
                cell_img.save(output_dir.join(&filename))
                    .map_err(|e| format!("セル保存エラー {filename}: {e}"))?;

                let check_filename = format!("R{row:02}C{col:02}_I{cell_idx}_check.png");
                check_img.save(output_dir.join(&check_filename))
                    .map_err(|e| format!("チェック欄保存エラー {check_filename}: {e}"))?;

                log!(
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

            let (adopted, reason) = judge_adoption(&slots);

            if adopted.is_empty() {
                total_empty += 1;
            } else {
                total_adopted += 1;
            }

            if !adopted.is_empty() || slots.iter().any(|s| !s.is_empty) {
                log!(
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

    log!("\n  文字サマリー: 採用={total_adopted}, 空={total_empty}, 合計={}", results.len());
    Ok(results)
}

/// WASM用: 全48文字を処理（ファイル保存なし）
pub fn extract_and_judge_in_memory(img: &RgbaImage) -> Result<Vec<CharResult>, String> {
    let border_margin = 1.0;
    let crop_size = layout::CELL_SIZE - border_margin * 2.0;
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

    let mut results = Vec::new();
    let mut total_adopted = 0usize;
    let mut total_empty = 0usize;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }
            let mut slots = Vec::new();

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
                let cell_img = crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px);

                let black_ratio = measure_inner_black_ratio(&cell_img, 0.2);
                let is_empty = black_ratio < 0.02;

                let check_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let check_px_y = layout::mm_to_px(mm_y + layout::CELL_SIZE + border_margin * 0.5).round() as u32;
                let check_w = layout::mm_to_px(layout::CELL_SIZE - border_margin * 2.0).round() as u32;
                let check_h = layout::mm_to_px(layout::CHECK_HEIGHT - border_margin).round() as u32;
                let check_img = crop_region(img, check_px_x, check_px_y, check_w, check_h);

                let (check_mark, check_density) = analyze_check_mark(&check_img);

                log!(
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

            let (adopted, reason) = judge_adoption(&slots);

            if adopted.is_empty() {
                total_empty += 1;
            } else {
                total_adopted += 1;
            }

            if !adopted.is_empty() || slots.iter().any(|s| !s.is_empty) {
                log!(
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

    log!("  文字サマリー: 採用={total_adopted}, 空={total_empty}, 合計={}", results.len());
    Ok(results)
}

/// セル画像を切り出して返す（WASM用）
pub fn extract_cell_image(img: &RgbaImage, row: usize, col: usize, cell_index: usize) -> RgbaImage {
    let border_margin = 1.0;
    let crop_size = layout::CELL_SIZE - border_margin * 2.0;
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

    let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_index);
    let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
    let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
    crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px)
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
/// Sauvola適応的二値化で黒ピクセルを判定
fn analyze_check_mark(check_img: &RgbaImage) -> (CheckMark, f64) {
    let w = check_img.width();
    let h = check_img.height();
    if w == 0 || h == 0 {
        return (CheckMark::Empty, 0.0);
    }

    let gray = rgba_to_gray(check_img);
    let gray = apply_clahe(&gray, w, h);
    let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
    let binary = morphological_open_close(&binary, w, h);

    let total = w * h;
    let black_count = binary.iter().filter(|&&v| v == 0).count() as u32;
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

/// 内側領域の黒ピクセル率を計測（Sauvola適応的二値化）
fn measure_inner_black_ratio(img: &RgbaImage, margin_ratio: f64) -> f64 {
    let w = img.width();
    let h = img.height();
    let margin_x = (w as f64 * margin_ratio).round() as u32;
    let margin_y = (h as f64 * margin_ratio).round() as u32;

    let end_x = w.saturating_sub(margin_x);
    let end_y = h.saturating_sub(margin_y);
    if margin_x >= end_x || margin_y >= end_y {
        return 0.0;
    }

    // 画像全体でSauvola二値化（局所的な閾値を正しく計算するため）
    let gray = rgba_to_gray(img);
    let gray = apply_clahe(&gray, w, h);
    let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
    let binary = morphological_open_close(&binary, w, h);

    // 内側領域のみカウント
    let mut black_count = 0u32;
    let mut total = 0u32;

    for y in margin_y..end_y {
        for x in margin_x..end_x {
            total += 1;
            if binary[(y * w + x) as usize] == 0 {
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

// ── CLAHE（ローカルコントラスト正規化） ──

const CLAHE_GRID: u32 = 4;   // タイル分割数
const CLAHE_CLIP: f64 = 3.0; // クリッピング係数

/// グレースケール画像にCLAHE（Contrast Limited Adaptive Histogram Equalization）を適用
fn apply_clahe(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    if w == 0 || h == 0 {
        return vec![];
    }

    let grid = CLAHE_GRID;
    let tile_w = w / grid;
    let tile_h = h / grid;

    if tile_w == 0 || tile_h == 0 {
        return gray.to_vec();
    }

    // 各タイルのLUT（ルックアップテーブル）を計算
    let mut luts = vec![[0u8; 256]; (grid * grid) as usize];

    for ty in 0..grid {
        for tx in 0..grid {
            let x0 = tx * tile_w;
            let y0 = ty * tile_h;
            let x1 = if tx == grid - 1 { w } else { x0 + tile_w };
            let y1 = if ty == grid - 1 { h } else { y0 + tile_h };

            // ヒストグラム計算
            let mut hist = [0u32; 256];
            let mut pixel_count = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    hist[gray[(y * w + x) as usize] as usize] += 1;
                    pixel_count += 1;
                }
            }

            if pixel_count == 0 {
                continue;
            }

            // クリッピング
            let clip_limit = (CLAHE_CLIP * pixel_count as f64 / 256.0).max(1.0) as u32;
            let mut excess = 0u32;
            for bin in hist.iter_mut() {
                if *bin > clip_limit {
                    excess += *bin - clip_limit;
                    *bin = clip_limit;
                }
            }
            // 超過分を均等再配分
            let per_bin = excess / 256;
            let remainder = (excess % 256) as usize;
            for (i, bin) in hist.iter_mut().enumerate() {
                *bin += per_bin;
                if i < remainder {
                    *bin += 1;
                }
            }

            // CDF計算 → LUT生成
            let mut cdf = [0u32; 256];
            cdf[0] = hist[0];
            for i in 1..256 {
                cdf[i] = cdf[i - 1] + hist[i];
            }
            let cdf_min = cdf.iter().copied().find(|&v| v > 0).unwrap_or(0);
            let denom = pixel_count.saturating_sub(cdf_min);

            let lut = &mut luts[(ty * grid + tx) as usize];
            for i in 0..256 {
                if denom == 0 {
                    lut[i] = i as u8;
                } else {
                    lut[i] = ((cdf[i].saturating_sub(cdf_min) as f64 * 255.0 / denom as f64)
                        .round()
                        .clamp(0.0, 255.0)) as u8;
                }
            }
        }
    }

    // バイリニア補間で最終出力を生成
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let val = gray[(y * w + x) as usize] as usize;

            // タイル中心座標を基準にした相対位置を計算
            // タイル中心: (tx + 0.5) * tile_w
            let fx = (x as f64 / tile_w as f64) - 0.5;
            let fy = (y as f64 / tile_h as f64) - 0.5;

            let tx0 = (fx.floor() as i32).clamp(0, grid as i32 - 1) as u32;
            let ty0 = (fy.floor() as i32).clamp(0, grid as i32 - 1) as u32;
            let tx1 = (tx0 + 1).min(grid - 1);
            let ty1 = (ty0 + 1).min(grid - 1);

            let ax = (fx - fx.floor()).clamp(0.0, 1.0);
            let ay = (fy - fy.floor()).clamp(0.0, 1.0);

            let v00 = luts[(ty0 * grid + tx0) as usize][val] as f64;
            let v10 = luts[(ty0 * grid + tx1) as usize][val] as f64;
            let v01 = luts[(ty1 * grid + tx0) as usize][val] as f64;
            let v11 = luts[(ty1 * grid + tx1) as usize][val] as f64;

            let top = v00 * (1.0 - ax) + v10 * ax;
            let bot = v01 * (1.0 - ax) + v11 * ax;
            let result = top * (1.0 - ay) + bot * ay;

            out[(y * w + x) as usize] = result.round().clamp(0.0, 255.0) as u8;
        }
    }

    out
}

// ── Sauvola 適応的二値化 ──

/// Sauvola法パラメータ
const SAUVOLA_K: f64 = 0.2;       // 感度パラメータ（文書画像の標準値）
const SAUVOLA_WINDOW: u32 = 15;   // 局所ウィンドウの一辺（300DPI、セル153×153px程度に適切）

/// RGBA画像をグレースケール配列に変換
fn rgba_to_gray(img: &RgbaImage) -> Vec<u8> {
    let w = img.width();
    let h = img.height();
    let mut gray = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            let lum = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
            gray.push(lum);
        }
    }
    gray
}

/// Sauvola法による適応的二値化
/// Integral Image（累積和テーブル）で局所平均・分散をO(1)計算
/// 閾値: T = mean * (1 + k * (std_dev / R - 1))
fn sauvola_binarize(gray: &[u8], w: u32, h: u32, k: f64, window_size: u32) -> Vec<u8> {
    let r_const = 128.0;
    let half = (window_size / 2) as i32;

    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }

    // Integral Image（累積和）と Integral Image^2（二乗累積和）を構築
    let mut sum = vec![0i64; n];
    let mut sq_sum = vec![0i64; n];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let val = gray[idx] as i64;
            let val_sq = val * val;

            let left = if x > 0 { sum[idx - 1] } else { 0 };
            let up = if y > 0 { sum[((y - 1) * w + x) as usize] } else { 0 };
            let diag = if x > 0 && y > 0 { sum[((y - 1) * w + x - 1) as usize] } else { 0 };
            sum[idx] = val + left + up - diag;

            let left_sq = if x > 0 { sq_sum[idx - 1] } else { 0 };
            let up_sq = if y > 0 { sq_sum[((y - 1) * w + x) as usize] } else { 0 };
            let diag_sq = if x > 0 && y > 0 { sq_sum[((y - 1) * w + x - 1) as usize] } else { 0 };
            sq_sum[idx] = val_sq + left_sq + up_sq - diag_sq;
        }
    }

    // 各ピクセルの閾値を計算
    let mut binary = vec![255u8; n]; // デフォルト白

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let x0 = (x - half).max(0);
            let y0 = (y - half).max(0);
            let x1 = (x + half).min(w as i32 - 1);
            let y1 = (y + half).min(h as i32 - 1);

            let count = ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64;

            let s = rect_sum(&sum, w, x0, y0, x1, y1) as f64;
            let s2 = rect_sum(&sq_sum, w, x0, y0, x1, y1) as f64;

            let mean = s / count;
            let variance = (s2 / count - mean * mean).max(0.0);
            let std_dev = variance.sqrt();

            // Sauvola閾値: T = mean * (1 + k * (std/R - 1))
            let threshold = mean * (1.0 + k * (std_dev / r_const - 1.0));

            let idx = (y as u32 * w + x as u32) as usize;
            let val = gray[idx] as f64;
            if val < threshold {
                binary[idx] = 0; // 黒
            }
        }
    }

    binary
}

/// Integral Imageから矩形領域の合計値を取得
fn rect_sum(integral: &[i64], w: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> i64 {
    let w = w as i32;
    let br = integral[(y1 * w + x1) as usize];
    let tl = if x0 > 0 && y0 > 0 { integral[((y0 - 1) * w + x0 - 1) as usize] } else { 0 };
    let top = if y0 > 0 { integral[((y0 - 1) * w + x1) as usize] } else { 0 };
    let left = if x0 > 0 { integral[(y1 * w + x0 - 1) as usize] } else { 0 };
    br + tl - top - left
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

// ── モルフォロジカル処理 ──

/// 3×3カーネルのErosion（収縮）: 近傍に白(255)があれば白にする
/// 黒領域を収縮させ、孤立黒ノイズを除去する
fn morphological_erode(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }
    let mut out = vec![0u8; n];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut has_white = false;
            'kernel: for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        // 画像外は白とみなす
                        has_white = true;
                        break 'kernel;
                    }
                    if binary[(ny as u32 * w + nx as u32) as usize] == 255 {
                        has_white = true;
                        break 'kernel;
                    }
                }
            }
            let idx = (y as u32 * w + x as u32) as usize;
            out[idx] = if has_white { 255 } else { 0 };
        }
    }
    out
}

/// 3×3カーネルのDilation（膨張）: 近傍に黒(0)があれば黒にする
/// 黒領域を膨張させ、孤立白ノイズを埋める
fn morphological_dilate(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }
    let mut out = vec![255u8; n];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut has_black = false;
            'kernel: for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    if binary[(ny as u32 * w + nx as u32) as usize] == 0 {
                        has_black = true;
                        break 'kernel;
                    }
                }
            }
            let idx = (y as u32 * w + x as u32) as usize;
            out[idx] = if has_black { 0 } else { 255 };
        }
    }
    out
}

/// Opening(Erode→Dilate)→Closing(Dilate→Erode)の一連処理
/// Opening: 小さな黒ノイズを除去、Closing: 小さな白ノイズを埋める
pub(crate) fn morphological_open_close(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    // Opening: Erode → Dilate
    let opened = morphological_dilate(&morphological_erode(binary, w, h), w, h);
    // Closing: Dilate → Erode
    morphological_erode(&morphological_dilate(&opened, w, h), w, h)
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
        // モルフォロジカル処理に耐えるよう、3px以上の太さのブロックを配置
        let mut img = make_uniform_image(100, 100, Rgba([255, 255, 255, 255]));
        let total = 100 * 100;
        let target_black = (total as f64 * 0.05) as usize;
        // 5×5ブロックを複数配置（各25px、20ブロックで500px = 5%）
        let mut count = 0usize;
        'outer: for by in 0..4u32 {
            for bx in 0..5u32 {
                if count >= target_black { break 'outer; }
                for dy in 0..5u32 {
                    for dx in 0..5u32 {
                        let x = bx * 20 + dx + 2;
                        let y = by * 20 + dy + 2;
                        if x < 100 && y < 100 {
                            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                            count += 1;
                        }
                    }
                }
            }
        }
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Check);
        assert!(density >= 0.02 && density <= 0.15, "density={density}");
    }

    #[test]
    fn check_mark_cross_for_dense_black() {
        // 密度20%程度 → Cross（>15%）
        // モルフォロジカル処理に耐えるよう、太いブロックを配置
        let mut img = make_uniform_image(100, 100, Rgba([255, 255, 255, 255]));
        let total = 100 * 100;
        let target_black = (total as f64 * 0.20) as usize;
        // 10×10ブロックを複数配置（各100px、20ブロックで2000px = 20%）
        let mut count = 0usize;
        'outer: for by in 0..5u32 {
            for bx in 0..5u32 {
                if count >= target_black { break 'outer; }
                for dy in 0..10u32 {
                    for dx in 0..10u32 {
                        let x = bx * 20 + dx;
                        let y = by * 20 + dy;
                        if x < 100 && y < 100 {
                            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                            count += 1;
                        }
                    }
                }
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
        // 均一黒画像: Sauvolaではコントラストがないため「背景」と判定 → 黒比率0
        // これはSauvola法の正しい挙動（固定閾値とは異なる）
        let img = make_uniform_image(100, 100, Rgba([0, 0, 0, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert!((ratio - 0.0).abs() < 0.01, "ratio={ratio} should be ~0.0 (Sauvola: uniform=no contrast)");
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

    // ── Sauvola 二値化テスト ──

    #[test]
    fn sauvola_uniform_white() {
        // 均一白画像 → 全白（黒ピクセルなし）
        let w = 50u32;
        let h = 50u32;
        let gray = vec![255u8; (w * h) as usize];
        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
        let black_count = binary.iter().filter(|&&v| v == 0).count();
        assert_eq!(black_count, 0, "均一白画像は全白であるべき");
    }

    #[test]
    fn sauvola_uniform_black() {
        // 均一黒画像 → Sauvolaでは均一領域にコントラストがないため全白になる
        // （mean=0, threshold=0, val<threshold は偽）
        // これはSauvola法の正しい挙動: 均一領域は「背景」と判定される
        let w = 50u32;
        let h = 50u32;
        let gray = vec![0u8; (w * h) as usize];
        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
        let black_count = binary.iter().filter(|&&v| v == 0).count();
        assert_eq!(black_count, 0, "均一黒画像はSauvolaでは全白（コントラストなし）");
    }

    #[test]
    fn sauvola_detects_text_on_both_halves() {
        // 左半分が暗い背景(100) + 暗い文字(30)、右半分が明るい背景(230) + 暗い文字(30)
        // Sauvolaの本領: 両方の「文字」部分を黒として検出する
        let w = 100u32;
        let h = 50u32;
        let mut gray = vec![0u8; (w * h) as usize];

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if x < 50 {
                    // 左半分: 暗い背景
                    gray[idx] = 100;
                } else {
                    // 右半分: 明るい背景
                    gray[idx] = 230;
                }
            }
        }

        // 左半分に「文字」（暗いピクセル）を配置
        for y in 20..30 {
            for x in 20..30 {
                gray[(y * w + x) as usize] = 30;
            }
        }

        // 右半分に「文字」（暗いピクセル）を配置
        for y in 20..30 {
            for x in 70..80 {
                gray[(y * w + x) as usize] = 30;
            }
        }

        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);

        // 左半分の文字領域に黒ピクセルがある
        let left_text_black = (20..30u32).flat_map(|y| (20..30u32).map(move |x| (y, x)))
            .filter(|&(y, x)| binary[(y * w + x) as usize] == 0)
            .count();

        // 右半分の文字領域に黒ピクセルがある
        let right_text_black = (20..30u32).flat_map(|y| (70..80u32).map(move |x| (y, x)))
            .filter(|&(y, x)| binary[(y * w + x) as usize] == 0)
            .count();

        assert!(left_text_black > 50, "左半分の文字を検出すべき（検出={left_text_black}/100）");
        assert!(right_text_black > 50, "右半分の文字を検出すべき（検出={right_text_black}/100）");
    }

    #[test]
    fn sauvola_empty_image() {
        // 0x0画像 → 空のVecを返す
        let binary = sauvola_binarize(&[], 0, 0, SAUVOLA_K, SAUVOLA_WINDOW);
        assert!(binary.is_empty());
    }

    #[test]
    fn rect_sum_single_pixel_at_origin() {
        // 3x3画像: 全ピクセル値1のIntegral Image
        // sum = [[1,2,3],[2,4,6],[3,6,9]]
        let integral: Vec<i64> = vec![1, 2, 3, 2, 4, 6, 3, 6, 9];
        // (0,0)〜(0,0)の1ピクセル = 1
        assert_eq!(rect_sum(&integral, 3, 0, 0, 0, 0), 1);
        // (0,0)〜(2,2)の全体 = 9
        assert_eq!(rect_sum(&integral, 3, 0, 0, 2, 2), 9);
        // (1,1)〜(2,2)の右下4ピクセル = 4
        assert_eq!(rect_sum(&integral, 3, 1, 1, 2, 2), 4);
    }

    // ── モルフォロジカル処理テスト ──

    #[test]
    fn erode_removes_isolated_black_pixel() {
        // 白背景に孤立1pxの黒点 → Erodeで消える
        let w = 5u32;
        let h = 5u32;
        let mut binary = vec![255u8; (w * h) as usize];
        binary[(2 * w + 2) as usize] = 0; // 中央に黒1px
        let result = morphological_erode(&binary, w, h);
        // 孤立黒点は周囲が白なので白に変わる
        assert_eq!(result[(2 * w + 2) as usize], 255, "孤立黒点がErodeで消えるべき");
        // 全体が白であること
        assert!(result.iter().all(|&v| v == 255), "全ピクセルが白であるべき");
    }

    #[test]
    fn dilate_removes_isolated_white_pixel() {
        // 黒背景に孤立1pxの白点 → Dilateで消える
        let w = 5u32;
        let h = 5u32;
        let mut binary = vec![0u8; (w * h) as usize];
        binary[(2 * w + 2) as usize] = 255; // 中央に白1px
        let result = morphological_dilate(&binary, w, h);
        // 孤立白点は周囲が黒なので黒に変わる
        assert_eq!(result[(2 * w + 2) as usize], 0, "孤立白点がDilateで消えるべき");
        // 全体が黒であること
        assert!(result.iter().all(|&v| v == 0), "全ピクセルが黒であるべき");
    }

    #[test]
    fn open_close_removes_noise_preserves_stroke() {
        // 白背景に太いストローク（3×3黒ブロック）+ 孤立黒ノイズ1px
        let w = 10u32;
        let h = 10u32;
        let mut binary = vec![255u8; (w * h) as usize];

        // 3×3の黒ブロック（太いストローク）を (3,3)-(5,5) に配置
        for y in 3..6u32 {
            for x in 3..6u32 {
                binary[(y * w + x) as usize] = 0;
            }
        }
        // 孤立黒ノイズ1px を (0,0) に配置
        binary[0] = 0;

        let result = morphological_open_close(&binary, w, h);

        // 孤立ノイズは消えるべき
        assert_eq!(result[0], 255, "孤立黒ノイズがOpeningで消えるべき");

        // 太いストロークの中心は保持されるべき
        assert_eq!(result[(4 * w + 4) as usize], 0, "ストローク中心は保持されるべき");
    }

    #[test]
    fn morphological_empty_image() {
        // 0x0画像 → 空のVecを返す
        assert!(morphological_erode(&[], 0, 0).is_empty());
        assert!(morphological_dilate(&[], 0, 0).is_empty());
        assert!(morphological_open_close(&[], 0, 0).is_empty());
    }

    // ── CLAHE テスト ──

    #[test]
    fn clahe_uniform_image() {
        // 均一画像（全ピクセル128）→ 出力も均一
        let w = 64u32;
        let h = 64u32;
        let gray = vec![128u8; (w * h) as usize];
        let result = apply_clahe(&gray, w, h);
        assert_eq!(result.len(), (w * h) as usize);
        let first = result[0];
        assert!(result.iter().all(|&v| v == first), "均一画像のCLAHE出力は均一であるべき");
    }

    #[test]
    fn clahe_preserves_dimensions() {
        // 入出力のサイズが同じ
        let w = 80u32;
        let h = 60u32;
        let gray: Vec<u8> = (0..(w * h)).map(|i| (i % 256) as u8).collect();
        let result = apply_clahe(&gray, w, h);
        assert_eq!(result.len(), gray.len(), "CLAHE出力サイズが入力と一致すべき");
    }

    #[test]
    fn clahe_improves_contrast() {
        // 低コントラスト画像（128±10）→ ダイナミックレンジが広がる
        let w = 64u32;
        let h = 64u32;
        let gray: Vec<u8> = (0..(w * h))
            .map(|i| (118 + (i % 21)) as u8) // 118..=138
            .collect();
        let input_min = *gray.iter().min().unwrap();
        let input_max = *gray.iter().max().unwrap();
        let input_range = input_max - input_min; // 20

        let result = apply_clahe(&gray, w, h);
        let output_min = *result.iter().min().unwrap();
        let output_max = *result.iter().max().unwrap();
        let output_range = output_max - output_min;

        assert!(
            output_range > input_range,
            "CLAHEでダイナミックレンジが広がるべき: input={input_range}, output={output_range}"
        );
    }
}
