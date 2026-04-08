/// セル切り出し + 空判定
use image::{RgbaImage, Rgba};
use crate::layout;
use std::path::Path;

/// セル切り出し結果
#[derive(Debug)]
#[allow(dead_code)]
pub struct CellResult {
    pub row: usize,
    pub col: usize,
    pub cell_index: usize,
    pub is_empty: bool,
    pub black_ratio: f64,
}

/// 全48セルを切り出して個別PNGで保存
pub fn extract_cells(img: &RgbaImage, output_dir: &Path) -> Result<Vec<CellResult>, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("セル出力ディレクトリ作成エラー: {}", e))?;

    let inner_size_px = layout::mm_to_px(layout::INNER_SIZE).round() as u32;
    let inner_offset = (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0; // 2.5mm
    let mut results = Vec::new();
    let mut empty_count = 0u32;
    let mut filled_count = 0u32;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);
                // 内枠領域の左上座標（外枠からinnerOffsetだけ内側）
                let px_x = layout::mm_to_px(mm_x + inner_offset).round() as u32;
                let px_y = layout::mm_to_px(mm_y + inner_offset).round() as u32;

                // 内枠領域のみ切り出し（INNER_SIZE = 10mm）
                let mut cell_img = RgbaImage::new(inner_size_px, inner_size_px);
                for dy in 0..inner_size_px {
                    for dx in 0..inner_size_px {
                        let sx = px_x + dx;
                        let sy = px_y + dy;
                        if sx < img.width() && sy < img.height() {
                            cell_img.put_pixel(dx, dy, *img.get_pixel(sx, sy));
                        } else {
                            cell_img.put_pixel(dx, dy, Rgba([255, 255, 255, 255]));
                        }
                    }
                }

                // 空判定: 内側60%領域で黒ピクセル2%未満=空
                let margin = (inner_size_px as f64 * 0.2).round() as u32;
                let inner_start = margin;
                let inner_end = inner_size_px - margin;
                let mut black_count = 0u32;
                let mut total = 0u32;

                for dy in inner_start..inner_end {
                    for dx in inner_start..inner_end {
                        total += 1;
                        let p = cell_img.get_pixel(dx, dy);
                        // 輝度で判定
                        let lum = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
                        if lum < 128 {
                            black_count += 1;
                        }
                    }
                }

                let black_ratio = if total > 0 {
                    black_count as f64 / total as f64
                } else {
                    0.0
                };
                let is_empty = black_ratio < 0.02;

                if is_empty {
                    empty_count += 1;
                } else {
                    filled_count += 1;
                }

                let filename = format!("R{:02}C{:02}_I{}.png", row, col, cell_idx);
                let filepath = output_dir.join(&filename);
                cell_img
                    .save(&filepath)
                    .map_err(|e| format!("セル保存エラー {}: {}", filename, e))?;

                println!(
                    "  R{:02}C{:02}_I{}: black={:.1}% {}",
                    row,
                    col,
                    cell_idx,
                    black_ratio * 100.0,
                    if is_empty { "(空)" } else { "(非空)" }
                );

                results.push(CellResult {
                    row,
                    col,
                    cell_index: cell_idx,
                    is_empty,
                    black_ratio,
                });
            }
        }
    }

    println!("\n  セルサマリー: 空={}, 非空={}, 合計={}", empty_count, filled_count, results.len());

    Ok(results)
}
