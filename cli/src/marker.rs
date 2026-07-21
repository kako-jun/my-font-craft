use crate::layout;
/// 二値化 + マーカー検出
use image::{GrayImage, Luma, Rgba, RgbaImage};

// ── マーカー候補選定の閾値（#132） ──

/// 隅ごとに保持する候補数（コーナー近さ順の distinct クラスタ上位K件、#132）。
/// 木目の机で撮影した実写真較正（Issue #132 実写真2枚）: 紙の外側の木目・机の縁が
/// 探索領域（25%マージン）の画像コーナーに近い側を埋め尽くすことがあり、実在マーカーは
/// 最大で17番目に近いクラスタだった（page1 TopLeft）。組み合わせ数は候補数の積 4 乗だが、
/// 各隅の生存候補（形状+紙白ゲート通過）は実写真で最大16件程度に収まり、K=30 でも
/// 数千comboで済む（1〜数秒未満）。
const CORNER_CANDIDATE_K: usize = 30;

/// 紙白アニュラス検証（#132・本命の防御）の内側/外側半径比率。
/// marker_px/2（マーカー半径相当）に掛けて環状領域の半径を決める。
/// 「紙の上のマーカー」は周囲が紙白、木目の節は周囲も木色（暗め・中間調）が続くため分離できる。
const ANNULUS_INNER_RATIO: f64 = 1.3;
const ANNULUS_OUTER_RATIO: f64 = 1.8;

/// 環状領域中、二値化で白（紙background）と判定される最小割合。
/// 平均輝度でなく割合にするのは、TLマーカー付近のグレーバー/見出し文字が
/// 平均を汚して誤爆させるため（#132）。
const ANNULUS_WHITE_RATIO_MIN: f64 = 0.5;

/// クアッド組み合わせスコアの重み（小さいほど良い、#132）。
/// 中心マーカー整合は射影変換で保存される対角線交点＝紙中心のアンカーで、
/// 相対不変量（アスペクト・対辺比）より判別力が強いため重く重み付けする。
const SCORE_WEIGHT_ASPECT: f64 = 1.0;
const SCORE_WEIGHT_SIDE_RATIO: f64 = 1.0;
const SCORE_WEIGHT_CENTER: f64 = 8.0;

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

/// パラボリック補間でマーカー中心をサブピクセル精度に精緻化
/// グレースケール画像の輝度プロファイルから放物線の頂点を求める
fn refine_center_parabolic(gray: &GrayImage, cx: f64, cy: f64) -> (f64, f64) {
    let icx = cx.round() as i32;
    let icy = cy.round() as i32;
    let w = gray.width() as i32;
    let h = gray.height() as i32;

    // 境界チェック: 補間に必要な±1ピクセルと、ノイズ耐性のための±2行/列が必要
    if icx < 2 || icy < 2 || icx >= w - 2 || icy >= h - 2 {
        return (cx, cy);
    }

    // X方向: icy-2..=icy+2 の5行について放物線フィットし、中央値を取る
    let mut dx_values: Vec<f64> = Vec::new();
    for row_offset in -2i32..=2 {
        let y = (icy + row_offset) as u32;
        let left = gray.get_pixel((icx - 1) as u32, y)[0] as f64;
        let center = gray.get_pixel(icx as u32, y)[0] as f64;
        let right = gray.get_pixel((icx + 1) as u32, y)[0] as f64;
        let denom = 2.0 * (left + right - 2.0 * center);
        if denom.abs() > 1e-6 {
            let dx = (left - right) / denom;
            if dx.abs() < 1.0 {
                dx_values.push(dx);
            }
        }
    }

    // Y方向: icx-2..=icx+2 の5列について放物線フィットし、中央値を取る
    let mut dy_values: Vec<f64> = Vec::new();
    for col_offset in -2i32..=2 {
        let x = (icx + col_offset) as u32;
        let top = gray.get_pixel(x, (icy - 1) as u32)[0] as f64;
        let center = gray.get_pixel(x, icy as u32)[0] as f64;
        let bottom = gray.get_pixel(x, (icy + 1) as u32)[0] as f64;
        let denom = 2.0 * (top + bottom - 2.0 * center);
        if denom.abs() > 1e-6 {
            let dy = (top - bottom) / denom;
            if dy.abs() < 1.0 {
                dy_values.push(dy);
            }
        }
    }

    // 中央値を取得（ノイズ耐性）
    let refined_x = if dx_values.is_empty() {
        cx
    } else {
        dx_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        icx as f64 + dx_values[dx_values.len() / 2]
    };

    let refined_y = if dy_values.is_empty() {
        cy
    } else {
        dy_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        icy as f64 + dy_values[dy_values.len() / 2]
    };

    (refined_x, refined_y)
}

/// 画像境界に接触する黒領域を白で塗りつぶす（背景除去）
/// 実写画像で紙の外側（机・背景）がマーカーと結合するのを防ぐ。
/// 画像の4辺にある黒ピクセルから flood fill し、到達可能な黒ピクセルを全て白化する。
pub fn mask_border_background(binary: &mut GrayImage) {
    let w = binary.width();
    let h = binary.height();
    let mut stack: Vec<(u32, u32)> = Vec::new();

    // 4辺の黒ピクセルをシードにする
    for x in 0..w {
        if binary.get_pixel(x, 0)[0] == 0 {
            stack.push((x, 0));
        }
        if binary.get_pixel(x, h - 1)[0] == 0 {
            stack.push((x, h - 1));
        }
    }
    for y in 1..h - 1 {
        if binary.get_pixel(0, y)[0] == 0 {
            stack.push((0, y));
        }
        if binary.get_pixel(w - 1, y)[0] == 0 {
            stack.push((w - 1, y));
        }
    }

    // flood fill（4連結）
    let white = Luma([255u8]);
    // 先にシードを白化してキューに入れる
    for &(x, y) in &stack {
        binary.put_pixel(x, y, white);
    }

    while let Some((x, y)) = stack.pop() {
        let neighbors = [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ];
        for (nx, ny) in neighbors {
            if nx < w && ny < h && binary.get_pixel(nx, ny)[0] == 0 {
                binary.put_pixel(nx, ny, white);
                stack.push((nx, ny));
            }
        }
    }
}

/// 隅ごとの1候補（形状+紙白ゲートを通過したもの、#132）
#[derive(Debug, Clone)]
struct CornerCandidate {
    marker: DetectedMarker,
    /// コーナー近さ順での試行順位（1始まり）。ログ・デバッグ用
    seed_rank: usize,
}

/// マージ後ブロブ（bbox + ピクセル加重重心）
struct MergedBlob {
    bbox_min_x: u32,
    bbox_max_x: u32,
    bbox_min_y: u32,
    bbox_max_y: u32,
    centroid_x: f64,
    centroid_y: f64,
    total_area: u32,
    merged_count: usize,
}

/// 種ブロブ周辺（merge_radius 内）のブロブを統合し、bbox と重心を計算する。
/// 候補ごとに使うため detect_markers から切り出したヘルパー（#132）。
fn merge_blobs_near_seed(
    filtered: &[&Blob],
    seed_cx: f64,
    seed_cy: f64,
    merge_radius: f64,
) -> MergedBlob {
    let mut total_area = 0u32;
    let mut total_sum_x = 0.0f64;
    let mut total_sum_y = 0.0f64;
    let mut merged_count = 0usize;
    let mut m_min_x = u32::MAX;
    let mut m_max_x = 0u32;
    let mut m_min_y = u32::MAX;
    let mut m_max_y = 0u32;

    for b in filtered {
        let bcx = b.center_x();
        let bcy = b.center_y();
        let dist = ((bcx - seed_cx).powi(2) + (bcy - seed_cy).powi(2)).sqrt();
        if dist <= merge_radius {
            total_area += b.area;
            total_sum_x += b.sum_x;
            total_sum_y += b.sum_y;
            merged_count += 1;
            m_min_x = m_min_x.min(b.min_x);
            m_max_x = m_max_x.max(b.max_x);
            m_min_y = m_min_y.min(b.min_y);
            m_max_y = m_max_y.max(b.max_y);
        }
    }

    MergedBlob {
        bbox_min_x: m_min_x,
        bbox_max_x: m_max_x,
        bbox_min_y: m_min_y,
        bbox_max_y: m_max_y,
        centroid_x: total_sum_x / total_area as f64,
        centroid_y: total_sum_y / total_area as f64,
        total_area,
        merged_count,
    }
}

/// 環状領域（中心を bbox 中心、半径 inner_r〜outer_r）の白ピクセル比率を計算する（#132）。
/// 「紙の上のマーカー」は周囲が紙白、木目の節は周囲も木色（暗め・中間調）が続くため、
/// この比率で分離できる。画像外にはみ出す領域はサンプルから除外する。
fn annulus_white_ratio(binary: &GrayImage, cx: f64, cy: f64, inner_r: f64, outer_r: f64) -> f64 {
    let w = binary.width() as i32;
    let h = binary.height() as i32;
    let icx = cx.round() as i32;
    let icy = cy.round() as i32;
    let r_out = outer_r.ceil() as i32;

    let mut white = 0u32;
    let mut total = 0u32;
    for dy in -r_out..=r_out {
        for dx in -r_out..=r_out {
            let dist = ((dx * dx + dy * dy) as f64).sqrt();
            if dist < inner_r || dist > outer_r {
                continue;
            }
            let px = icx + dx;
            let py = icy + dy;
            if px < 0 || py < 0 || px >= w || py >= h {
                continue;
            }
            total += 1;
            if binary.get_pixel(px as u32, py as u32)[0] != 0 {
                white += 1;
            }
        }
    }

    if total == 0 {
        return 0.0;
    }
    white as f64 / total as f64
}

/// クアッド対角線（TL-BR と TR-BL）の交点を求める。
/// 射影変換は接続関係を保存するため、矩形の対角線交点（≒紙中心）はこの交点に写る。
fn diagonal_intersection(
    tl: (f64, f64),
    tr: (f64, f64),
    bl: (f64, f64),
    br: (f64, f64),
) -> Option<(f64, f64)> {
    let (x1, y1) = tl;
    let (x2, y2) = br;
    let (x3, y3) = tr;
    let (x4, y4) = bl;
    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;
    Some((x1 + t * (x2 - x1), y1 + t * (y2 - y1)))
}

/// クアッド組み合わせのスコア（#132）。小さいほど良い。
/// a) クアッドのアスペクト比の期待値（expected_quad_aspect）からの偏差
/// b) 対辺長比（上下・左右）の 1.0 からの偏差
/// c) 中心マーカー整合: 対角線交点と中心マーカー位置の距離（対角線長で正規化）。
///    中心マーカー未検出時はこの項を除外し a, b のみで判定する。
fn combo_score(quad: &[DetectedMarker; 4], center_hint: Option<&DetectedMarker>) -> f64 {
    let tl = (quad[0].cx, quad[0].cy);
    let tr = (quad[1].cx, quad[1].cy);
    let bl = (quad[2].cx, quad[2].cy);
    let br = (quad[3].cx, quad[3].cy);

    let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let top_w = dist(tl, tr);
    let bottom_w = dist(bl, br);
    let left_h = dist(tl, bl);
    let right_h = dist(tr, br);
    let mean_w = (top_w + bottom_w) / 2.0;
    let mean_h = (left_h + right_h) / 2.0;

    let expected = expected_quad_aspect();
    let aspect_dev = if mean_h > 0.0 {
        ((mean_w / mean_h) / expected - 1.0).abs()
    } else {
        0.0
    };
    let side_dev = (top_w / bottom_w - 1.0).abs() + (left_h / right_h - 1.0).abs();

    let mut score = SCORE_WEIGHT_ASPECT * aspect_dev + SCORE_WEIGHT_SIDE_RATIO * side_dev;

    if let Some(center) = center_hint {
        if let Some((ix, iy)) = diagonal_intersection(tl, tr, bl, br) {
            let diag_len = dist(tl, br).max(dist(tr, bl));
            if diag_len > 1e-6 {
                let center_dev =
                    ((ix - center.cx).powi(2) + (iy - center.cy).powi(2)).sqrt() / diag_len;
                score += SCORE_WEIGHT_CENTER * center_dev;
            }
        }
    }

    score
}

/// 四隅マーカーを検出する。25%マージン領域を探索
/// ブロブの面積・形状でフィルタし、隅ごとに上位 K 件を候補化。
/// 各候補を形状ゲート（#115）＋紙白アニュラスゲート（#132）で足切りし、
/// 生き残った候補の全組み合わせをクアッド幾何ゲート＋スコアリングで評価して最良を採用する。
/// `center_hint` は事前検出した中心マーカー（未検出なら None）。スコアリングの
/// 最強のアンカーとして使うが、必須ではない（None ならアスペクト・対辺比のみで判定）。
pub fn detect_markers(
    binary: &GrayImage,
    gray: &GrayImage,
    center_hint: Option<&DetectedMarker>,
) -> Result<[DetectedMarker; 4], String> {
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

    let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
    // 塗りつぶし円の期待面積（px²）
    let expected_filled_area = std::f64::consts::PI * (marker_px / 2.0).powi(2);
    // 個別ブロブのフィルタ範囲（アウトラインの弧も拾うが、巨大ブロブは除外）
    let min_blob_area = 30u32;
    let max_blob_area = (expected_filled_area * 5.0) as u32;
    let merge_radius = marker_px * 1.0; // 1.0倍に縮小（1.5では文字を巻き込む）
    let annulus_inner = (marker_px / 2.0) * ANNULUS_INNER_RATIO;
    let annulus_outer = (marker_px / 2.0) * ANNULUS_OUTER_RATIO;

    let corner_points: [(f64, f64); 4] = [
        (0.0, 0.0),
        (w as f64, 0.0),
        (0.0, h as f64),
        (w as f64, h as f64),
    ];

    let mut per_corner_candidates: Vec<Vec<CornerCandidate>> = Vec::with_capacity(4);

    for (i, (name, x0, y0, x1, y1)) in regions.iter().enumerate() {
        let blobs = extract_blobs(binary, *x0, *y0, *x1, *y1);
        let (corner_x, corner_y) = corner_points[i];

        // フィルタ: 面積範囲 + アスペクト比（細長いバー・罫線を除外）
        let filtered: Vec<&Blob> = blobs
            .iter()
            .filter(|b| {
                // aspect は width/height。円マーカーは ≈1.0。下限 0.2 だと縦長グレーバーの
                // 最暗ステップ（5mm 幅 / 24.4mm 高 = 0.205）が四隅探索領域に入ってすり抜ける。
                // 円は中程度の透視歪みでも 0.5〜2.0 に収まるため、0.35〜3.0 に締めて細長片を除外する。
                b.area >= min_blob_area
                    && b.area <= max_blob_area
                    && b.aspect_ratio() > 0.35
                    && b.aspect_ratio() < 3.0
            })
            .collect();

        log!(
            "  {} 探索領域: ({},{})..({},{}) ブロブ数={} フィルタ後={}",
            name,
            x0,
            y0,
            x1,
            y1,
            blobs.len(),
            filtered.len()
        );

        if filtered.is_empty() {
            return Err(format!(
                "{} マーカーが検出できませんでした（ブロブ数={}, フィルタ通過=0）",
                name,
                blobs.len()
            ));
        }

        // コーナー近さ順にソート（インデックス列。ブロブ本体は filtered のまま）
        let mut order: Vec<usize> = (0..filtered.len()).collect();
        order.sort_by(|&a, &b| {
            let da = (filtered[a].center_x() - corner_x).powi(2)
                + (filtered[a].center_y() - corner_y).powi(2);
            let db = (filtered[b].center_x() - corner_x).powi(2)
                + (filtered[b].center_y() - corner_y).powi(2);
            da.partial_cmp(&db).unwrap()
        });

        // 上位 K 件を候補化（#132）。木目のように近距離に同一クラスタの小ブロブが
        // 大量に密集していると、K 件が全て同じクラスタの重複候補で埋まり、
        // 別クラスタ（実在マーカー等）に到達できなくなる。一度マージ評価した
        // クラスタに属するブロブは visited にして、以降は新規候補として数えない
        // （K は「distinct クラスタ」の上限として機能する）。
        let mut visited = vec![false; filtered.len()];
        let mut candidates: Vec<CornerCandidate> = Vec::new();
        let mut tried = 0usize;
        for &idx in &order {
            if visited[idx] {
                continue;
            }
            if tried >= CORNER_CANDIDATE_K {
                break;
            }
            tried += 1;
            let seed_cx = filtered[idx].center_x();
            let seed_cy = filtered[idx].center_y();

            // マージ＋重心計算（bbox中心ではなくピクセル重心を使う）
            let merged = merge_blobs_near_seed(&filtered, seed_cx, seed_cy, merge_radius);
            let bbox_w = (merged.bbox_max_x - merged.bbox_min_x + 1) as f64;
            let bbox_h = (merged.bbox_max_y - merged.bbox_min_y + 1) as f64;

            // このクラスタに実際に属した全ブロブを visited にする（重複候補防止）
            for (j, b) in filtered.iter().enumerate() {
                let dist =
                    ((b.center_x() - seed_cx).powi(2) + (b.center_y() - seed_cy).powi(2)).sqrt();
                if dist <= merge_radius {
                    visited[j] = true;
                }
            }

            // 形状ゲート（#115。#132で「致命エラー」から「候補の足切りゲート」に降格）:
            // 落ちても即エラーにせず次点候補へ回す。
            if let Err(e) = validate_marker_shape(name, bbox_w, bbox_h, marker_px) {
                log!("  {name} 候補{tried}: 形状ゲート棄却（{e}）");
                continue;
            }

            // 紙白アニュラスゲート（#132・本命の防御）: 木目の節など紙外の誤検出は
            // 周囲も暗い/中間調なので、形状ゲートを通っても弾ける。
            let bbox_cx = (merged.bbox_min_x + merged.bbox_max_x) as f64 / 2.0;
            let bbox_cy = (merged.bbox_min_y + merged.bbox_max_y) as f64 / 2.0;
            let white_ratio =
                annulus_white_ratio(binary, bbox_cx, bbox_cy, annulus_inner, annulus_outer);
            if white_ratio < ANNULUS_WHITE_RATIO_MIN {
                log!("  {name} 候補{tried}: 紙白ゲート棄却 白比率={white_ratio:.2}（閾値={ANNULUS_WHITE_RATIO_MIN}）");
                continue;
            }

            // パラボリック補間でサブピクセル精緻化
            let (refined_x, refined_y) =
                refine_center_parabolic(gray, merged.centroid_x, merged.centroid_y);

            log!(
                "  {name} 候補{tried}: centroid=({:.1}, {:.1}) → refined=({refined_x:.2}, {refined_y:.2}) area={} merged={}ブロブ bbox={bbox_w:.0}x{bbox_h:.0} 白比率={white_ratio:.2}",
                merged.centroid_x, merged.centroid_y, merged.total_area, merged.merged_count
            );

            candidates.push(CornerCandidate {
                marker: DetectedMarker {
                    cx: refined_x,
                    cy: refined_y,
                    area: merged.total_area,
                },
                seed_rank: tried,
            });
        }

        log!(
            "  {name} 候補確定: {}/{}（形状/紙白ゲート通過/試行）",
            candidates.len(),
            tried
        );

        if candidates.is_empty() {
            return Err(format!(
                "{name} マーカーが検出できませんでした（候補{tried}件、形状/紙白検証を通過した候補なし）"
            ));
        }

        per_corner_candidates.push(candidates);
    }

    // 生き残った候補の全組み合わせをクアッド幾何ゲート＋スコアリングで評価（#132）
    // (採用クアッド, スコア, 各隅の候補seed_rank) の組
    type BestCombo = ([DetectedMarker; 4], f64, (usize, usize, usize, usize));
    let mut best: Option<BestCombo> = None;
    let mut first_quad_err: Option<String> = None;
    let mut combos_tried = 0usize;

    for tl in &per_corner_candidates[0] {
        for tr in &per_corner_candidates[1] {
            for bl in &per_corner_candidates[2] {
                for br in &per_corner_candidates[3] {
                    combos_tried += 1;
                    let quad = [
                        tl.marker.clone(),
                        tr.marker.clone(),
                        bl.marker.clone(),
                        br.marker.clone(),
                    ];
                    if let Err(e) = validate_marker_quad(&quad) {
                        if first_quad_err.is_none() {
                            first_quad_err = Some(e);
                        }
                        continue;
                    }
                    let score = combo_score(&quad, center_hint);
                    if best.as_ref().map(|(_, s, _)| score < *s).unwrap_or(true) {
                        best = Some((
                            quad,
                            score,
                            (tl.seed_rank, tr.seed_rank, bl.seed_rank, br.seed_rank),
                        ));
                    }
                }
            }
        }
    }

    log!(
        "  クアッド組み合わせ探索: 候補数 TL={} TR={} BL={} BR={} 試行={combos_tried} 生存={}",
        per_corner_candidates[0].len(),
        per_corner_candidates[1].len(),
        per_corner_candidates[2].len(),
        per_corner_candidates[3].len(),
        if best.is_some() { "あり" } else { "なし" }
    );

    match best {
        Some((quad, score, ranks)) => {
            log!(
                "  採用: TL(候補{})=({:.1},{:.1}) TR(候補{})=({:.1},{:.1}) BL(候補{})=({:.1},{:.1}) BR(候補{})=({:.1},{:.1}) score={score:.4}",
                ranks.0, quad[0].cx, quad[0].cy,
                ranks.1, quad[1].cx, quad[1].cy,
                ranks.2, quad[2].cx, quad[2].cy,
                ranks.3, quad[3].cx, quad[3].cy,
            );
            Ok(quad)
        }
        None => {
            // 検出後クアッド幾何検証（#115）: 全組み合わせが幾何破綻で棄却された。
            // ここで棄却しないと、デタラメな centroid のまま透視補正が進み、
            // QR が読めず「不鮮明」に誤診断される。
            Err(first_quad_err.unwrap_or_else(|| {
                "四隅マーカーの配置が不正です（マーカー誤検出の可能性）。四隅のマーカーが隠れず紙全体が写るように撮影してください。".to_string()
            }))
        }
    }
}

/// 四隅マーカー候補ブロブの外接矩形が「円らしい」形状かを検証する（#115）。
///
/// #132 でクアッド全体を棄却する「致命エラー」から、候補ごとの「足切りゲート」に降格。
/// 落ちた候補は次点候補に乗り換えるだけで、ここで紙外の誤検出を確実に分離しきる必要はない
/// （本命の防御は紙白アニュラス検証 annulus_white_ratio + 組み合わせスコアリング）。
///
/// 幾何クアッド不変量（アスペクト・対辺比・面積）は平行移動・スケール不変なので、
/// 「1点だけ別ブロブを誤検出」を「急な透視の正規スキャン」から原理的に分離できない。
/// 一方ブロブ形状は透視不変（円は中程度の透視でも概ね円のまま）なので、角度のついた
/// 実写を誤棄却せずに誤検出だけを弾ける。
///
/// 較正（300dpi フィクスチャ実測・第1パス）:
/// - 実在マーカー（塗り円 TL / リング TR・BL・BR）: bbox_aspect 0.94〜1.02、
///   bbox 各辺 ≈ 85〜104px（marker_px≈94.5 の 0.9〜1.1 倍）
/// - 欠落時の誤検出ブロブ: タイトル文字列 = bbox 110x27（aspect=4.07）等、円から大きく逸脱
///   （fill_ratio は塗り円 0.785 / リング 0.07 と幅広く、リングと誤検出を分離できないので不使用）
pub fn validate_marker_shape(
    name: &str,
    bbox_w: f64,
    bbox_h: f64,
    marker_px: f64,
) -> Result<(), String> {
    let aspect = bbox_w / bbox_h;
    // 円の外接矩形は正方形（≈1.0）。中程度の透視（〜30°）でも短縮率 cos30≈0.87 に留まり
    // 0.87〜1.15 程度。0.6〜1.67 に締めて横長テキスト列・縦長罫線を弾く（実写は誤棄却しない）。
    if !(0.6..=1.667).contains(&aspect) {
        return Err(format!(
            "{name} 四隅マーカーらしい形状が見つかりません（マーカー誤検出の可能性・外接矩形が円形でない: 縦横比={aspect:.2}）。四隅のマーカーが隠れたり塗りつぶされたりせず紙全体が写るように撮影してください。"
        ));
    }
    // 大きさサニティ: マーカー実寸に対し極端に大小のブロブ（巨大セル領域・微小ノイズ）を弾く。
    // 実測は 0.9〜1.1 倍。透視で多少大きくなる余地を見て 0.45〜2.5 倍と広く取る。
    let wr = bbox_w / marker_px;
    let hr = bbox_h / marker_px;
    if !(0.45..=2.5).contains(&wr) || !(0.45..=2.5).contains(&hr) {
        return Err(format!(
            "{name} 四隅マーカーらしい形状が見つかりません（マーカー誤検出の可能性・大きさが不正: {bbox_w:.0}x{bbox_h:.0}px, 期待≈{marker_px:.0}px）。四隅のマーカーが隠れたり塗りつぶされたりせず紙全体が写るように撮影してください。"
        ));
    }
    Ok(())
}

/// テンプレート既知の四隅マーカー矩形アスペクト（mean_width / mean_height）。
/// layout の MARKER_TL/TR/BL/BR + marker_center から算出し、レイアウト変更に自動追従する
/// （ハードコードしない）。TL≈(7,7) TR≈(205,7) BL≈(7,290.915) BR≈(205,290.915) mm → ≈0.697。
fn expected_quad_aspect() -> f64 {
    let tl = layout::marker_center(&layout::MARKER_TL);
    let tr = layout::marker_center(&layout::MARKER_TR);
    let bl = layout::marker_center(&layout::MARKER_BL);
    let br = layout::marker_center(&layout::MARKER_BR);
    let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let mean_width = (dist(tl, tr) + dist(bl, br)) / 2.0;
    let mean_height = (dist(tl, bl) + dist(tr, br)) / 2.0;
    mean_width / mean_height
}

/// 検出した四隅マーカー中心 [TL, TR, BL, BR] が妥当なマーカー矩形かを検証する（#115）。
///
/// これは「潰れ・点の入れ替わり・約90°の取り違え」といった gross failure だけを捕らえる
/// 粗いサニティであり、誤検出そのものの分離は validate_marker_shape（透視不変）が担う。
/// バンドは正規フィクスチャに加え中程度に角度のついた実写（〜25〜30°傾き）も必ず通す広さに取る。
/// 逸脱時は 'マーカー' を含む Err を返し、inferFailedStage が marker 段階へ振る。
pub fn validate_marker_quad(markers: &[DetectedMarker; 4]) -> Result<(), String> {
    let tl = (markers[0].cx, markers[0].cy);
    let tr = (markers[1].cx, markers[1].cy);
    let bl = (markers[2].cx, markers[2].cy);
    let br = (markers[3].cx, markers[3].cy);

    let dist = |a: (f64, f64), b: (f64, f64)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();

    let top_w = dist(tl, tr);
    let bottom_w = dist(bl, br);
    let left_h = dist(tl, bl);
    let right_h = dist(tr, br);

    // 非退化: 各辺が十分な長さを持つこと（マーカーが同一点に潰れていない）
    let min_side = 50.0_f64;
    if top_w < min_side || bottom_w < min_side || left_h < min_side || right_h < min_side {
        return Err(format!(
            "四隅マーカーの配置が不正です（マーカー誤検出の可能性・辺が短すぎる: top={top_w:.0} bottom={bottom_w:.0} left={left_h:.0} right={right_h:.0}）。四隅のマーカーが隠れず紙全体が写るように撮影してください。"
        ));
    }

    // 非退化・符号（点の入れ替わり検出）: TL→TR→BR→BL の符号付き面積のみを見る
    // （各頂点の凸性までは検査しない）。画像座標系（y 下向き）でこの巡回は時計回り = 正。
    // 符号が反転／小さすぎれば点が入れ替わっている or 潰れている。
    let poly = [tl, tr, br, bl];
    let mut area2 = 0.0;
    for i in 0..4 {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % 4];
        area2 += x1 * y2 - x2 * y1;
    }
    let signed_area = area2 / 2.0;
    // 画像座標系（y 下向き）では TL→TR→BR→BL は時計回り = 正の符号。
    // 期待面積（mean_width * mean_height）の下限割合。誤検出でクアッドが潰れる/符号反転すると弾く。
    let mean_w = (top_w + bottom_w) / 2.0;
    let mean_h = (left_h + right_h) / 2.0;
    let area_floor = 0.25 * mean_w * mean_h;
    if signed_area <= area_floor {
        // 正で十分大きくなければ退化 or 自己交差 or 点の入れ替わり。
        return Err(format!(
            "四隅マーカーの配置が不正です（マーカー誤検出の可能性・退化した四角形: 面積={signed_area:.0}）。四隅のマーカーが隠れず紙全体が写るように撮影してください。"
        ));
    }

    // 対辺比: 透視歪みは対辺比を偏らせるが、限度がある。正規フィクスチャは 1.0 付近。
    // 対辺比は透視で大きく偏りうる（ピンホール: 比 1.2 ≈ 15°傾き, 1.4 ≈ 20°傾き）。
    // ここを締めると角度のついた正規スキャンを誤棄却するので、gross failure だけを捕らえる
    // 広い帯（0.6〜1.667 ≈ 〜30°傾き）に留める。誤検出の分離は validate_marker_shape が担う。
    let w_ratio = top_w / bottom_w;
    let h_ratio = left_h / right_h;
    let ratio_lo = 0.6;
    let ratio_hi = 1.0 / ratio_lo;
    if !(ratio_lo..=ratio_hi).contains(&w_ratio) || !(ratio_lo..=ratio_hi).contains(&h_ratio) {
        return Err(format!(
            "四隅マーカーの配置が不正です（マーカー誤検出の可能性・対辺比が異常: 上下={w_ratio:.2} 左右={h_ratio:.2}）。四隅のマーカーが隠れず紙全体が写るように撮影してください。"
        ));
    }

    // テンプレートアスペクトとの照合。誤検出で1点がページ内側に寄ると大きくずれる。
    let expected = expected_quad_aspect();
    let aspect = mean_w / mean_h;
    let aspect_tol = 0.22; // 相対許容（±22%）
    let lo = expected * (1.0 - aspect_tol);
    let hi = expected * (1.0 + aspect_tol);
    log!(
        "  クアッド検証: aspect={aspect:.3} (期待={expected:.3}, 許容={lo:.3}..{hi:.3}) 対辺比 上下={w_ratio:.3} 左右={h_ratio:.3} 面積={signed_area:.0}"
    );
    if !(lo..=hi).contains(&aspect) {
        return Err(format!(
            "四隅マーカーの配置が不正です（マーカー誤検出の可能性・アスペクト比が異常: 実測={aspect:.3} 期待={expected:.3}）。四隅のマーカーが隠れず紙全体が写るように撮影してください。"
        ));
    }

    Ok(())
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
        log!(
            "  中心マーカー検出: centroid=({:.1}, {:.1}) area={} fill_ratio={:.2}",
            b.center_x(),
            b.center_y(),
            b.area,
            b.fill_ratio()
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
                if px >= 0
                    && py >= 0
                    && (px as u32) < binary.width()
                    && (py as u32) < binary.height()
                {
                    total += 1;
                    if binary.get_pixel(px as u32, py as u32)[0] == 0 {
                        black_count += 1;
                    }
                }
            }
        }

        let density = if total > 0 {
            black_count as f64 / total as f64
        } else {
            0.0
        };
        log!("  マーカー[{i}]: 密度={density:.3} (黒={black_count}/{total})");
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

    log!("  filled マーカー位置: [{tl_index}], 回転角度: {rotation}°");

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
pub fn reorder_markers(
    markers: &[DetectedMarker; 4],
    tl_index: usize,
    rotation: u32,
    img_w: u32,
    img_h: u32,
) -> [DetectedMarker; 4] {
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
        DetectedMarker {
            cx: nx,
            cy: ny,
            area: m.area,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(cx: f64, cy: f64) -> DetectedMarker {
        DetectedMarker { cx, cy, area: 100 }
    }

    /// レイアウトのテンプレートマーカー中心（mm→px）から妥当な四隅クアッドを組む
    fn template_quad() -> [DetectedMarker; 4] {
        let c = |m: &layout::MarkerDef| {
            let (x, y) = layout::marker_center(m);
            mk(layout::mm_to_px(x), layout::mm_to_px(y))
        };
        [
            c(&layout::MARKER_TL),
            c(&layout::MARKER_TR),
            c(&layout::MARKER_BL),
            c(&layout::MARKER_BR),
        ]
    }

    // --- クアッド幾何（gross failure サニティ。誤検出そのものの分離は shape が担う） ---

    #[test]
    fn valid_template_quad_passes() {
        assert!(validate_marker_quad(&template_quad()).is_ok());
    }

    #[test]
    fn moderate_perspective_quad_passes() {
        // ~28°傾き相当: 上辺 1600 / 下辺 2400（対辺比 0.667 = 下辺が上辺の1.5倍）の台形。
        // 締めすぎた帯だと誤棄却するケース。緩い幾何サニティは必ず通さねばならない。
        let q = [
            mk(450.0, 100.0),   // TL
            mk(2050.0, 100.0),  // TR（上辺 1600）
            mk(50.0, 3300.0),   // BL
            mk(2450.0, 3300.0), // BR（下辺 2400）
        ];
        assert!(
            validate_marker_quad(&q).is_ok(),
            "中程度の透視（~28°）が誤棄却された: {:?}",
            validate_marker_quad(&q)
        );
    }

    #[test]
    fn degenerate_short_side_quad_fails() {
        // 4点が同一位置に潰れている（辺長ゼロ）→ 辺が短すぎる分岐
        let q = [
            mk(500.0, 500.0),
            mk(500.0, 500.0),
            mk(500.0, 500.0),
            mk(500.0, 500.0),
        ];
        let err = validate_marker_quad(&q).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("辺が短すぎる"),
            "err={err}"
        );
    }

    #[test]
    fn point_swapped_quad_fails() {
        // TL と TR を入れ替えると自己交差（bowtie）→ 符号付き面積が退化
        let t = template_quad();
        let q = [t[1].clone(), t[0].clone(), t[2].clone(), t[3].clone()];
        let err = validate_marker_quad(&q).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("退化"),
            "err={err}"
        );
    }

    #[test]
    fn wrong_aspect_quad_fails() {
        // ほぼ正方形（aspect≈1.0）はテンプレート 0.697 から大きく外れる（~90°取り違え相当）
        let q = [
            mk(100.0, 100.0),
            mk(2100.0, 100.0),
            mk(100.0, 2100.0),
            mk(2100.0, 2100.0),
        ];
        let err = validate_marker_quad(&q).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("アスペクト"),
            "err={err}"
        );
    }

    // --- ブロブ形状スコア（透視不変・本命の誤検出分離） ---

    const MARKER_PX: f64 = 94.5; // mm_to_px(MARKER_SIZE=8) @300dpi

    #[test]
    fn circle_like_blob_shape_passes() {
        // 実在マーカー（塗り円・リングとも外接矩形は正方形 ≈ marker_px）
        assert!(validate_marker_shape("T", 94.0, 94.0, MARKER_PX).is_ok());
        // 中程度透視で少しつぶれても通る
        assert!(validate_marker_shape("T", 104.0, 89.0, MARKER_PX).is_ok());
    }

    #[test]
    fn text_row_blob_shape_fails() {
        // 欠落時に掴むタイトル文字列の外接矩形（実測 110x27, aspect=4.07）
        let err = validate_marker_shape("TopLeft", 110.0, 27.0, MARKER_PX).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("形状") && err.contains("縦横比"),
            "err={err}"
        );
    }

    #[test]
    fn vertical_line_blob_shape_fails() {
        // 縦長の罫線残渣（aspect ≈ 0.1）も円形でないとして棄却
        let err = validate_marker_shape("T", 20.0, 200.0, MARKER_PX).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("縦横比"),
            "err={err}"
        );
    }

    #[test]
    fn oversized_blob_shape_fails() {
        // 正方形でもマーカー実寸の 3 倍超なら別物（巨大セル領域など）
        let err = validate_marker_shape("T", 300.0, 300.0, MARKER_PX).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("大きさ"),
            "err={err}"
        );
    }
}
