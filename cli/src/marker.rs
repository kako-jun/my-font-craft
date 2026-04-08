/// 二値化 + マーカー検出
use image::{GrayImage, Luma, RgbaImage, Rgba};
use crate::layout;

/// 大津の方法で閾値を算出
pub fn otsu_threshold(gray: &GrayImage) -> u8 {
    let mut histogram = [0u64; 256];
    for pixel in gray.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total = gray.width() as f64 * gray.height() as f64;
    let mut sum_total = 0.0f64;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += i as f64 * count as f64;
    }

    let mut sum_bg = 0.0f64;
    let mut weight_bg = 0.0f64;
    let mut max_variance = 0.0f64;
    let mut best_threshold = 0u8;

    for (t, &count) in histogram.iter().enumerate() {
        weight_bg += count as f64;
        if weight_bg == 0.0 {
            continue;
        }
        let weight_fg = total - weight_bg;
        if weight_fg == 0.0 {
            break;
        }
        sum_bg += t as f64 * count as f64;
        let mean_bg = sum_bg / weight_bg;
        let mean_fg = (sum_total - sum_bg) / weight_fg;
        let variance = weight_bg * weight_fg * (mean_bg - mean_fg) * (mean_bg - mean_fg);
        if variance > max_variance {
            max_variance = variance;
            best_threshold = t as u8;
        }
    }

    best_threshold
}

/// 二値化（閾値以下 → 黒(0)、閾値超 → 白(255)）
pub fn binarize(gray: &GrayImage, threshold: u8) -> GrayImage {
    let mut binary = GrayImage::new(gray.width(), gray.height());
    for (x, y, pixel) in gray.enumerate_pixels() {
        let v = if pixel[0] <= threshold { 0u8 } else { 255u8 };
        binary.put_pixel(x, y, Luma([v]));
    }
    binary
}

/// ブロブ（連結成分）
#[derive(Debug, Clone)]
pub struct Blob {
    pub area: u32,
    pub sum_x: f64,
    pub sum_y: f64,
    pub min_x: u32,
    pub max_x: u32,
    pub min_y: u32,
    pub max_y: u32,
}

#[allow(dead_code)]
impl Blob {
    pub fn center_x(&self) -> f64 {
        self.sum_x / self.area as f64
    }
    pub fn center_y(&self) -> f64 {
        self.sum_y / self.area as f64
    }
    pub fn width(&self) -> u32 {
        self.max_x - self.min_x + 1
    }
    pub fn height(&self) -> u32 {
        self.max_y - self.min_y + 1
    }
    pub fn aspect_ratio(&self) -> f64 {
        self.width() as f64 / self.height() as f64
    }
    pub fn fill_ratio(&self) -> f64 {
        self.area as f64 / (self.width() as f64 * self.height() as f64)
    }
}

/// 4連結 union-find で黒ピクセルの連結成分を抽出
fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]]; // path splitting
        i = parent[i];
    }
    i
}

fn union(parent: &mut [usize], rank: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra == rb {
        return;
    }
    if rank[ra] < rank[rb] {
        parent[ra] = rb;
    } else if rank[ra] > rank[rb] {
        parent[rb] = ra;
    } else {
        parent[rb] = ra;
        rank[ra] += 1;
    }
}

/// 指定領域内の黒ピクセルブロブを抽出
pub fn extract_blobs(binary: &GrayImage, x0: u32, y0: u32, x1: u32, y1: u32) -> Vec<Blob> {
    let w = (x1 - x0) as usize;
    let h = (y1 - y0) as usize;
    let n = w * h;
    let mut parent = (0..n).collect::<Vec<_>>();
    let mut rank = vec![0usize; n];

    // ラベリング（4連結）
    for iy in 0..h {
        for ix in 0..w {
            let px = (x0 + ix as u32).min(binary.width() - 1);
            let py = (y0 + iy as u32).min(binary.height() - 1);
            if binary.get_pixel(px, py)[0] != 0 {
                continue; // 白ピクセルはスキップ
            }
            let idx = iy * w + ix;
            // 上
            if iy > 0 {
                let px2 = (x0 + ix as u32).min(binary.width() - 1);
                let py2 = (y0 + (iy - 1) as u32).min(binary.height() - 1);
                if binary.get_pixel(px2, py2)[0] == 0 {
                    union(&mut parent, &mut rank, idx, (iy - 1) * w + ix);
                }
            }
            // 左
            if ix > 0 {
                let px2 = (x0 + (ix - 1) as u32).min(binary.width() - 1);
                let py2 = (y0 + iy as u32).min(binary.height() - 1);
                if binary.get_pixel(px2, py2)[0] == 0 {
                    union(&mut parent, &mut rank, idx, iy * w + (ix - 1));
                }
            }
        }
    }

    // ブロブ集約
    use std::collections::HashMap;
    let mut blobs: HashMap<usize, Blob> = HashMap::new();

    for iy in 0..h {
        for ix in 0..w {
            let px = (x0 + ix as u32).min(binary.width() - 1);
            let py = (y0 + iy as u32).min(binary.height() - 1);
            if binary.get_pixel(px, py)[0] != 0 {
                continue;
            }
            let idx = iy * w + ix;
            let root = find(&mut parent, idx);
            let abs_x = x0 + ix as u32;
            let abs_y = y0 + iy as u32;

            let blob = blobs.entry(root).or_insert(Blob {
                area: 0,
                sum_x: 0.0,
                sum_y: 0.0,
                min_x: abs_x,
                max_x: abs_x,
                min_y: abs_y,
                max_y: abs_y,
            });
            blob.area += 1;
            blob.sum_x += abs_x as f64;
            blob.sum_y += abs_y as f64;
            blob.min_x = blob.min_x.min(abs_x);
            blob.max_x = blob.max_x.max(abs_x);
            blob.min_y = blob.min_y.min(abs_y);
            blob.max_y = blob.max_y.max(abs_y);
        }
    }

    blobs.into_values().collect()
}

/// マーカー検出結果
#[derive(Debug, Clone)]
pub struct DetectedMarker {
    pub cx: f64,
    pub cy: f64,
    pub area: u32,
}

/// 四隅マーカーを検出する。25%マージン領域を探索
/// ブロブの面積・形状でフィルタし、重心（centroid）を返す
pub fn detect_markers(binary: &GrayImage) -> Result<[DetectedMarker; 4], String> {
    let w = binary.width();
    let h = binary.height();
    let margin_x = (w as f64 * 0.25) as u32;
    let margin_y = (h as f64 * 0.25) as u32;

    let regions = [
        ("TopLeft", 0, 0, margin_x, margin_y),
        ("TopRight", w - margin_x, 0, w, margin_y),
        ("BottomLeft", 0, h - margin_y, margin_x, h),
        ("BottomRight", w - margin_x, h - margin_y, w, h),
    ];

    let mut markers = Vec::new();

    let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
    // 塗りつぶし円の期待面積（px²）
    let expected_filled_area = std::f64::consts::PI * (marker_px / 2.0).powi(2);
    // 個別ブロブのフィルタ範囲（アウトラインの弧も拾うが、巨大ブロブは除外）
    let min_blob_area = 30u32;
    let max_blob_area = (expected_filled_area * 5.0) as u32;

    let corner_points: [(f64, f64); 4] = [
        (0.0, 0.0),
        (w as f64, 0.0),
        (0.0, h as f64),
        (w as f64, h as f64),
    ];

    for (i, (name, x0, y0, x1, y1)) in regions.iter().enumerate() {
        let blobs = extract_blobs(binary, *x0, *y0, *x1, *y1);
        let (corner_x, corner_y) = corner_points[i];

        // フィルタ: 面積範囲 + アスペクト比（細長いバー・罫線を除外）
        let filtered: Vec<&Blob> = blobs
            .iter()
            .filter(|b| {
                b.area >= min_blob_area
                    && b.area <= max_blob_area
                    && b.aspect_ratio() > 0.2
                    && b.aspect_ratio() < 5.0
            })
            .collect();

        println!(
            "  {} 探索領域: ({},{})..({},{}) ブロブ数={} フィルタ後={}",
            name, x0, y0, x1, y1, blobs.len(), filtered.len()
        );

        if filtered.is_empty() {
            return Err(format!(
                "{} マーカーが検出できませんでした（ブロブ数={}, フィルタ通過=0）",
                name, blobs.len()
            ));
        }

        // コーナーに最も近いブロブを種として選ぶ
        let seed = filtered.iter().min_by(|a, b| {
            let da = (a.center_x() - corner_x).powi(2) + (a.center_y() - corner_y).powi(2);
            let db = (b.center_x() - corner_x).powi(2) + (b.center_y() - corner_y).powi(2);
            da.partial_cmp(&db).unwrap()
        }).unwrap();

        let seed_cx = seed.center_x();
        let seed_cy = seed.center_y();
        let merge_radius = marker_px * 1.0; // 1.0倍に縮小（1.5では文字を巻き込む）

        // マージ＋重心計算（bbox中心ではなくピクセル重心を使う）
        let mut total_area = 0u32;
        let mut total_sum_x = 0.0f64;
        let mut total_sum_y = 0.0f64;
        let mut merged_count = 0usize;

        for b in &filtered {
            let bcx = b.center_x();
            let bcy = b.center_y();
            let dist = ((bcx - seed_cx).powi(2) + (bcy - seed_cy).powi(2)).sqrt();
            if dist <= merge_radius {
                total_area += b.area;
                total_sum_x += b.sum_x;
                total_sum_y += b.sum_y;
                merged_count += 1;
            }
        }

        // 重心（ピクセル加重平均）
        let centroid_x = total_sum_x / total_area as f64;
        let centroid_y = total_sum_y / total_area as f64;

        println!(
            "  {name} マーカー: centroid=({centroid_x:.1}, {centroid_y:.1}) area={total_area} merged={merged_count}ブロブ",
        );
        markers.push(DetectedMarker {
            cx: centroid_x,
            cy: centroid_y,
            area: total_area,
        });
    }

    Ok([
        markers[0].clone(),
        markers[1].clone(),
        markers[2].clone(),
        markers[3].clone(),
    ])
}

/// 中心マーカーを検出する（ページ中央 ±10% の領域を探索）
pub fn detect_center_marker(binary: &GrayImage) -> Option<DetectedMarker> {
    let w = binary.width();
    let h = binary.height();
    let search_w = (w as f64 * 0.10) as u32;
    let search_h = (h as f64 * 0.10) as u32;
    let cx = w / 2;
    let cy = h / 2;

    let x0 = cx.saturating_sub(search_w);
    let y0 = cy.saturating_sub(search_h);
    let x1 = (cx + search_w).min(w);
    let y1 = (cy + search_h).min(h);

    let blobs = extract_blobs(binary, x0, y0, x1, y1);

    let center_px = layout::mm_to_px(layout::CENTER_MARKER_SIZE).round();
    let expected_area = center_px * center_px; // filled square

    // 面積が期待値の 20%〜500% で、ほぼ正方形のブロブを選ぶ
    let candidates: Vec<&Blob> = blobs
        .iter()
        .filter(|b| {
            let a = b.area as f64;
            a > expected_area * 0.2
                && a < expected_area * 5.0
                && b.aspect_ratio() > 0.5
                && b.aspect_ratio() < 2.0
                && b.fill_ratio() > 0.5 // 塗りつぶしマーカーなので充填率高い
        })
        .collect();

    // ページ中心に最も近い候補を選択
    let page_cx = w as f64 / 2.0;
    let page_cy = h as f64 / 2.0;

    let best = candidates.iter().min_by(|a, b| {
        let da = (a.center_x() - page_cx).powi(2) + (a.center_y() - page_cy).powi(2);
        let db = (b.center_x() - page_cx).powi(2) + (b.center_y() - page_cy).powi(2);
        da.partial_cmp(&db).unwrap()
    });

    best.map(|b| {
        println!(
            "  中心マーカー検出: centroid=({:.1}, {:.1}) area={} fill_ratio={:.2}",
            b.center_x(), b.center_y(), b.area, b.fill_ratio()
        );
        DetectedMarker {
            cx: b.center_x(),
            cy: b.center_y(),
            area: b.area,
        }
    })
}

/// マーカー検出位置を赤丸で可視化
pub fn draw_marker_overlay(img: &RgbaImage, markers: &[DetectedMarker; 4]) -> RgbaImage {
    let mut out = img.clone();
    let red = Rgba([255, 0, 0, 255]);
    let radius = 20i32;

    for m in markers {
        let cx = m.cx.round() as i32;
        let cy = m.cy.round() as i32;
        // 円を描画
        for angle in 0..360 {
            let rad = (angle as f64) * std::f64::consts::PI / 180.0;
            let px = cx + (radius as f64 * rad.cos()).round() as i32;
            let py = cy + (radius as f64 * rad.sin()).round() as i32;
            if px >= 0 && py >= 0 && (px as u32) < out.width() && (py as u32) < out.height() {
                out.put_pixel(px as u32, py as u32, red);
            }
        }
        // 十字
        for d in -radius..=radius {
            let px = (cx + d).max(0) as u32;
            let py = cy.max(0) as u32;
            if px < out.width() && py < out.height() {
                out.put_pixel(px, py, red);
            }
            let px = cx.max(0) as u32;
            let py = (cy + d).max(0) as u32;
            if px < out.width() && py < out.height() {
                out.put_pixel(px, py, red);
            }
        }
    }

    out
}

/// 向き検出: 各マーカー周辺の黒ピクセル密度を計測し、filledマーカー（TL）を判定
pub fn detect_orientation(
    binary: &GrayImage,
    markers: &[DetectedMarker; 4],
) -> Result<(usize, u32), String> {
    // 各マーカーの周辺密度を計測（マーカー中心から半径内の黒ピクセル数）
    let radius = 30u32; // 検査半径（px）
    let mut densities = Vec::new();

    for (i, m) in markers.iter().enumerate() {
        let cx = m.cx.round() as i32;
        let cy = m.cy.round() as i32;
        let mut black_count = 0u32;
        let mut total = 0u32;

        for dy in -(radius as i32)..=(radius as i32) {
            for dx in -(radius as i32)..=(radius as i32) {
                if dx * dx + dy * dy > (radius * radius) as i32 {
                    continue;
                }
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && (px as u32) < binary.width() && (py as u32) < binary.height() {
                    total += 1;
                    if binary.get_pixel(px as u32, py as u32)[0] == 0 {
                        black_count += 1;
                    }
                }
            }
        }

        let density = if total > 0 { black_count as f64 / total as f64 } else { 0.0 };
        println!("  マーカー[{i}]: 密度={density:.3} (黒={black_count}/{total})");
        densities.push((i, density));
    }

    // 最も密度が高い角をTL（filled）と判定
    densities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let tl_index = densities[0].0;

    // TLが[0]（= TopLeft領域で検出されたもの）なら回転不要（0度）
    // [1]=TopRight → 270度回転必要
    // [2]=BottomLeft → 90度回転必要
    // [3]=BottomRight → 180度回転必要
    let rotation = match tl_index {
        0 => 0u32,
        1 => 270,
        2 => 90,
        3 => 180,
        _ => unreachable!(),
    };

    println!("  filled マーカー位置: [{tl_index}], 回転角度: {rotation}°");

    Ok((tl_index, rotation))
}

/// 画像を90度単位で回転
pub fn rotate_image(img: &RgbaImage, degrees: u32) -> RgbaImage {
    match degrees {
        0 => img.clone(),
        90 => {
            let w = img.width();
            let h = img.height();
            let mut out = RgbaImage::new(h, w);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(h - 1 - y, x, *img.get_pixel(x, y));
                }
            }
            out
        }
        180 => {
            let w = img.width();
            let h = img.height();
            let mut out = RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(w - 1 - x, h - 1 - y, *img.get_pixel(x, y));
                }
            }
            out
        }
        270 => {
            let w = img.width();
            let h = img.height();
            let mut out = RgbaImage::new(h, w);
            for y in 0..h {
                for x in 0..w {
                    out.put_pixel(y, w - 1 - x, *img.get_pixel(x, y));
                }
            }
            out
        }
        _ => img.clone(),
    }
}

/// マーカー配列を回転に合わせて並べ替え（TL, TR, BL, BR の順に）
pub fn reorder_markers(markers: &[DetectedMarker; 4], tl_index: usize, rotation: u32, img_w: u32, img_h: u32) -> [DetectedMarker; 4] {
    if rotation == 0 {
        return markers.clone();
    }

    // 回転後のマーカー座標を変換
    let transform = |m: &DetectedMarker| -> DetectedMarker {
        let (nx, ny) = match rotation {
            90 => (img_h as f64 - 1.0 - m.cy, m.cx),
            180 => (img_w as f64 - 1.0 - m.cx, img_h as f64 - 1.0 - m.cy),
            270 => (m.cy, img_w as f64 - 1.0 - m.cx),
            _ => (m.cx, m.cy),
        };
        DetectedMarker { cx: nx, cy: ny, area: m.area }
    };

    // tl_index が回転後にTLになるように並べ替え
    let order = match tl_index {
        0 => [0, 1, 2, 3],
        1 => [1, 3, 0, 2], // TR→TL, BR→TR, TL→BL, BL→BR
        2 => [2, 0, 3, 1], // BL→TL, TL→TR, BR→BL, TR→BR
        3 => [3, 2, 1, 0], // BR→TL, BL→TR, TR→BL, TL→BR
        _ => unreachable!(),
    };

    [
        transform(&markers[order[0]]),
        transform(&markers[order[1]]),
        transform(&markers[order[2]]),
        transform(&markers[order[3]]),
    ]
}
