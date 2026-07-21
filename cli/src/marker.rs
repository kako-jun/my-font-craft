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

/// 補正後画像の局所再検出（#132フォローアップ）に使う探索半径の比率。
/// marker_px（テンプレート期待マーカーサイズ px）に掛けて窓の半径を決める。
/// 「2〜3×marker_px」の中間を取った値。木目机の wood-background フィクスチャで
/// 実写較正: ホモグラフィー後の残差はせいぜい数十px（TPS前でも通常 <marker_px）
/// なので、2.5倍の窓があれば実在マーカーは必ず窓内に収まる。
pub const LOCAL_SEARCH_RADIUS_RATIO: f64 = 2.5;

/// 期待位置近傍だけを探索してマーカーを再検出する（#132フォローアップ・補正後の再検出専用）。
///
/// 背景: 補正後画像では4隅の期待位置が layout の mm 座標から既知なので、初回検出
/// （detect_markers）と同じ「25%マージン領域の全域探索」は不要かつ有害になり得る。
/// wood-background フィクスチャ（machine #132 セルフレビュー指摘）では、補正後の
/// 再検出が全域探索のままだと同じ木目クラスタに再び引っ張られ、TopRight の候補が
/// 全滅して反復が未収束のまま打ち切られていた。
///
/// 各マーカーについて期待中心を中心とする一辺 2×search_radius_px の正方窓だけで
/// ブロブ抽出し、形状ゲート（#115, validate_marker_shape）を通過した候補のうち
/// 期待位置に最も近いものを採用する。
///
/// 紙白アニュラスゲート（annulus_white_ratio）はここでは適用しない: 窓が画像端
/// （紙の外）にかかると、アニュラスの外側リングが紙外領域を含んで誤爆し得る
/// （残差が残っている状態では期待位置が実際のマーカーより紙端寄りにズレることがある）。
/// 窓を期待位置近傍だけに絞ること自体が「遠く離れた木目等はそもそも候補にすらならない」
/// という強い防御になっており、全域探索版のような追加ゲートは不要と判断した。
pub fn detect_markers_near_expected(
    binary: &GrayImage,
    gray: &GrayImage,
    expected_centers_px: &[(f64, f64); 4],
    search_radius_px: f64,
) -> Result<[DetectedMarker; 4], String> {
    const NAMES: [&str; 4] = ["TopLeft", "TopRight", "BottomLeft", "BottomRight"];

    let w = binary.width();
    let h = binary.height();
    let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
    let expected_filled_area = std::f64::consts::PI * (marker_px / 2.0).powi(2);
    let min_blob_area = 30u32;
    let max_blob_area = (expected_filled_area * 5.0) as u32;
    let merge_radius = marker_px * 1.0;

    let mut markers: Vec<DetectedMarker> = Vec::with_capacity(4);

    for (i, &(ecx, ecy)) in expected_centers_px.iter().enumerate() {
        let name = NAMES[i];
        let x0 = (ecx - search_radius_px).max(0.0) as u32;
        let y0 = (ecy - search_radius_px).max(0.0) as u32;
        let x1 = ((ecx + search_radius_px).min(w as f64)) as u32;
        let y1 = ((ecy + search_radius_px).min(h as f64)) as u32;

        if x1 <= x0 || y1 <= y0 {
            return Err(format!(
                "{name} マーカーが検出できませんでした（局所探索窓が画像範囲外）"
            ));
        }

        let blobs = extract_blobs(binary, x0, y0, x1, y1);
        let filtered: Vec<&Blob> = blobs
            .iter()
            .filter(|b| {
                b.area >= min_blob_area
                    && b.area <= max_blob_area
                    && b.aspect_ratio() > 0.35
                    && b.aspect_ratio() < 3.0
            })
            .collect();

        log!(
            "  {name} 局所再探索: 窓=({x0},{y0})..({x1},{y1}) 期待=({ecx:.1},{ecy:.1}) ブロブ数={} フィルタ後={}",
            blobs.len(),
            filtered.len()
        );

        if filtered.is_empty() {
            return Err(format!(
                "{name} マーカーが検出できませんでした（局所窓内ブロブ数={}, フィルタ通過=0）",
                blobs.len()
            ));
        }

        // 期待位置に近い順に候補化し、形状ゲート（#115）を通過した最初の候補
        // （＝期待位置に最も近い候補）を採用する。全域版と異なり紙白アニュラス
        // ゲートは適用しない（理由は関数ドキュメント参照）。
        let mut order: Vec<usize> = (0..filtered.len()).collect();
        order.sort_by(|&a, &b| {
            let da =
                (filtered[a].center_x() - ecx).powi(2) + (filtered[a].center_y() - ecy).powi(2);
            let db =
                (filtered[b].center_x() - ecx).powi(2) + (filtered[b].center_y() - ecy).powi(2);
            da.partial_cmp(&db).unwrap()
        });

        let mut visited = vec![false; filtered.len()];
        let mut found: Option<DetectedMarker> = None;
        let mut tried = 0usize;

        for &idx in &order {
            if visited[idx] {
                continue;
            }
            tried += 1;
            let seed_cx = filtered[idx].center_x();
            let seed_cy = filtered[idx].center_y();
            let merged = merge_blobs_near_seed(&filtered, seed_cx, seed_cy, merge_radius);

            for (j, b) in filtered.iter().enumerate() {
                let dist =
                    ((b.center_x() - seed_cx).powi(2) + (b.center_y() - seed_cy).powi(2)).sqrt();
                if dist <= merge_radius {
                    visited[j] = true;
                }
            }

            let bbox_w = (merged.bbox_max_x - merged.bbox_min_x + 1) as f64;
            let bbox_h = (merged.bbox_max_y - merged.bbox_min_y + 1) as f64;

            if let Err(e) = validate_marker_shape(name, bbox_w, bbox_h, marker_px) {
                log!("  {name} 局所候補{tried}: 形状ゲート棄却（{e}）");
                continue;
            }

            let (refined_x, refined_y) =
                refine_center_parabolic(gray, merged.centroid_x, merged.centroid_y);
            log!(
                "  {name} 局所候補{tried}: 採用 centroid=({:.1}, {:.1}) → refined=({refined_x:.2}, {refined_y:.2}) bbox={bbox_w:.0}x{bbox_h:.0}",
                merged.centroid_x, merged.centroid_y
            );
            found = Some(DetectedMarker {
                cx: refined_x,
                cy: refined_y,
                area: merged.total_area,
            });
            break;
        }

        match found {
            Some(m) => markers.push(m),
            None => {
                return Err(format!(
                    "{name} マーカーが検出できませんでした（局所候補{tried}件、形状検証を通過した候補なし）"
                ));
            }
        }
    }

    Ok([
        markers[0].clone(),
        markers[1].clone(),
        markers[2].clone(),
        markers[3].clone(),
    ])
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

    // --- validate_marker_shape 境界値（#132・3点法） ---

    #[test]
    fn validate_marker_shape_aspect_lo_boundary() {
        // 下限 0.6（inclusive）。bbox_h=150 固定でサイズ比ゲートには触れない。
        assert!(
            validate_marker_shape("T", 89.9, 150.0, MARKER_PX).is_err(),
            "aspect=0.5993 は下限未満で棄却されるべき"
        );
        assert!(
            validate_marker_shape("T", 90.0, 150.0, MARKER_PX).is_ok(),
            "aspect=0.6 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_shape("T", 90.1, 150.0, MARKER_PX).is_ok(),
            "aspect=0.6007 は下限超で通るべき"
        );
    }

    #[test]
    fn validate_marker_shape_aspect_hi_boundary() {
        // 上限 1.667（inclusive）。bbox_h=100 固定でサイズ比ゲートには触れない。
        assert!(
            validate_marker_shape("T", 166.6, 100.0, MARKER_PX).is_ok(),
            "aspect=1.666 は上限未満で通るべき"
        );
        assert!(
            validate_marker_shape("T", 166.7, 100.0, MARKER_PX).is_ok(),
            "aspect=1.667 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_shape("T", 166.8, 100.0, MARKER_PX).is_err(),
            "aspect=1.668 は上限超で棄却されるべき"
        );
    }

    #[test]
    fn validate_marker_shape_size_ratio_lo_boundary() {
        // 下限 0.45（inclusive）。bbox_h=60 固定で aspect は 0.6〜1.667 の範囲内に収める
        // （wr=bbox_w/marker_px が境界を跨いでも aspect ゲートには触れないようにする）。
        assert!(
            validate_marker_shape("T", 42.4, 60.0, MARKER_PX).is_err(),
            "wr=0.4487 は下限未満で棄却されるべき"
        );
        assert!(
            validate_marker_shape("T", 42.525, 60.0, MARKER_PX).is_ok(),
            "wr=0.45 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_shape("T", 42.6, 60.0, MARKER_PX).is_ok(),
            "wr=0.4508 は下限超で通るべき"
        );
    }

    #[test]
    fn validate_marker_shape_size_ratio_hi_boundary() {
        // 上限 2.5（inclusive）。bbox_h=150 固定で aspect は範囲内に収める。
        assert!(
            validate_marker_shape("T", 236.1, 150.0, MARKER_PX).is_ok(),
            "wr=2.4984 は上限未満で通るべき"
        );
        assert!(
            validate_marker_shape("T", 236.25, 150.0, MARKER_PX).is_ok(),
            "wr=2.5 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_shape("T", 236.4, 150.0, MARKER_PX).is_err(),
            "wr=2.5016 は上限超で棄却されるべき"
        );
    }

    // --- validate_marker_quad 境界値（#132・3点法） ---

    #[test]
    fn validate_marker_quad_min_side_boundary() {
        // 上辺長 50.0px（inclusive）。他の辺・アスペクト・面積は十分余裕を持たせる。
        let q = |top_w: f64| [mk(0.0, 0.0), mk(top_w, 0.0), mk(0.0, 72.0), mk(50.0, 72.0)];
        assert!(
            validate_marker_quad(&q(49.9)).is_err(),
            "top_w=49.9 は下限未満で棄却されるべき"
        );
        assert!(
            validate_marker_quad(&q(50.0)).is_ok(),
            "top_w=50.0 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_quad(&q(50.1)).is_ok(),
            "top_w=50.1 は下限超で通るべき"
        );
    }

    #[test]
    fn validate_marker_quad_area_floor_boundary_le_fails() {
        // signed_area <= area_floor（0.25*mean_w*mean_h）で退化判定。<= の等号側を確認する。
        // 座標は「min_side・アスペクト・対辺比は全て許容帯内に収めたまま面積比だけを
        // area_floor 境界付近で振る」凸4角形をテンプレート矩形との線形補間（詳細はテスト作成
        // セッションの探索記録を参照）。signed_area/area_floor は t に対して非線形なので、
        // 事前計算した定数ではなく実際の validate_marker_quad を使って実行時に二分探索し、
        // 「最後に落ちる t（lo）」と「最初に通る t（hi）」を隣接する浮動小数点値まで詰める。
        // これにより <= の等号側（lo）が退化棄却されることを浮動小数点誤差に頼らず確認する。
        let q = |t: f64| {
            let lerp =
                |a: (f64, f64), b: (f64, f64)| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let a_tl = (0.0, 0.0);
            let a_tr = (60.22791507158993, 122.00427808013211);
            let a_bl = (146.10019783850225, 168.69732496263964);
            let a_br = (327.07822618877304, 177.81725999008614);
            let expected = 198.0 / 283.915;
            let w = 200.0;
            let l = w / expected;
            let b_tl = (0.0, 0.0);
            let b_tr = (w, 0.0);
            let b_bl = (0.0, l);
            let b_br = (w, l);
            let (tlx, tly) = lerp(a_tl, b_tl);
            let (trx, try_y) = lerp(a_tr, b_tr);
            let (blx, bly) = lerp(a_bl, b_bl);
            let (brx, bry) = lerp(a_br, b_br);
            [mk(tlx, tly), mk(trx, try_y), mk(blx, bly), mk(brx, bry)]
        };

        // t=0.15 は退化（ratio<1）、t=0.20 は通過（ratio>1）であることを事前調査済み。
        let mut lo = 0.15f64; // 退化側（Err）を保つ
        let mut hi = 0.20f64; // 通過側（Ok）を保つ
        assert!(
            validate_marker_quad(&q(lo)).is_err(),
            "探索区間の下端は退化のはず"
        );
        assert!(
            validate_marker_quad(&q(hi)).is_ok(),
            "探索区間の上端は通過のはず"
        );
        for _ in 0..100 {
            let mid = (lo + hi) / 2.0;
            if mid == lo || mid == hi {
                break; // 隣接する浮動小数点値まで収束
            }
            if validate_marker_quad(&q(mid)).is_err() {
                lo = mid;
            } else {
                hi = mid;
            }
        }

        let err_at_lo = validate_marker_quad(&q(lo)).unwrap_err();
        assert!(
            err_at_lo.contains("マーカー") && err_at_lo.contains("退化"),
            "area_floor境界の退化側（signed_area<=area_floor）は退化棄却されるべき: err={err_at_lo}"
        );
        assert!(
            validate_marker_quad(&q(hi)).is_ok(),
            "隣接する浮動小数点値まで詰めた通過側（signed_area>area_floor）は通るべき"
        );
    }

    #[test]
    fn validate_marker_quad_side_ratio_lo_boundary() {
        // 対辺比下限 0.6（inclusive）。bottom_w=100 固定、上辺のみ動かす。
        // H=120 でアスペクトをテンプレート期待値の許容帯に収める。
        let q = |top_w: f64| {
            [
                mk(-top_w / 2.0, 0.0),
                mk(top_w / 2.0, 0.0),
                mk(-50.0, 120.0),
                mk(50.0, 120.0),
            ]
        };
        assert!(
            validate_marker_quad(&q(59.9)).is_err(),
            "w_ratio=0.599 は下限未満で棄却されるべき"
        );
        assert!(
            validate_marker_quad(&q(60.0)).is_ok(),
            "w_ratio=0.6 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_quad(&q(60.1)).is_ok(),
            "w_ratio=0.601 は下限超で通るべき"
        );
    }

    #[test]
    fn validate_marker_quad_side_ratio_hi_boundary() {
        // 対辺比上限 1/0.6（inclusive）。bottom_w=64.0（2の累乗）固定、上辺=hi*64.0。
        // 2の累乗の乗除算は丸め誤差が生じないため、w_ratio=top_w/64.0 が
        // ソース側の ratio_hi = 1.0/0.6 と bit-exact に一致する
        // （100倍して割り戻すと二重丸めでずれる。2の累乗倍なら仮数部が変化せず可逆）。
        let hi = 1.0f64 / 0.6f64;
        let bottom_w = 64.0f64;
        let top_w_at = hi * bottom_w;
        // H=128 でアスペクトをテンプレート期待値の許容帯に収める（境界検証には無関係）。
        let q = |top_w: f64| {
            [
                mk(-top_w / 2.0, 0.0),
                mk(top_w / 2.0, 0.0),
                mk(-bottom_w / 2.0, 128.0),
                mk(bottom_w / 2.0, 128.0),
            ]
        };
        assert!(
            validate_marker_quad(&q(top_w_at - 0.1)).is_ok(),
            "w_ratio が上限未満は通るべき"
        );
        assert!(
            validate_marker_quad(&q(top_w_at)).is_ok(),
            "w_ratio=1/0.6 ちょうどは inclusive で通るべき"
        );
        assert!(
            validate_marker_quad(&q(top_w_at + 0.1)).is_err(),
            "w_ratio が上限超は棄却されるべき"
        );
    }

    // --- merge_blobs_near_seed（#132） ---

    #[test]
    fn merge_blobs_near_seed_radius_includes_near_excludes_far() {
        // 種(50,50)から半径100内の3ブロブを面積加重合成し、半径外の1ブロブは除外する。
        let mk_blob = |area: u32, cx: f64, cy: f64, min_x, max_x, min_y, max_y| Blob {
            area,
            sum_x: area as f64 * cx,
            sum_y: area as f64 * cy,
            min_x,
            max_x,
            min_y,
            max_y,
        };
        let near1 = mk_blob(10, 50.0, 50.0, 45, 55, 45, 55); // dist=0
        let near2 = mk_blob(20, 100.0, 50.0, 95, 105, 45, 55); // dist=50
        let near3 = mk_blob(30, 120.0, 80.0, 110, 130, 70, 90); // dist≈76.2
        let far = mk_blob(40, 300.0, 300.0, 290, 310, 290, 310); // dist≈353.6（半径外）
        let blobs = [&near1, &near2, &near3, &far];

        let merged = merge_blobs_near_seed(&blobs, 50.0, 50.0, 100.0);

        assert_eq!(
            merged.merged_count, 3,
            "半径外の1ブロブは統合対象から除外されるべき"
        );
        assert_eq!(merged.total_area, 60, "統合面積は半径内3ブロブの合計のみ");
        assert!(
            (merged.centroid_x - (6100.0 / 60.0)).abs() < 1e-9,
            "面積加重重心x: got={}",
            merged.centroid_x
        );
        assert!(
            (merged.centroid_y - 65.0).abs() < 1e-9,
            "面積加重重心y: got={}",
            merged.centroid_y
        );
        assert_eq!(merged.bbox_min_x, 45);
        assert_eq!(merged.bbox_max_x, 130, "半径外ブロブのbboxは含まれないべき");
        assert_eq!(merged.bbox_min_y, 45);
        assert_eq!(merged.bbox_max_y, 90);
    }

    // --- diagonal_intersection（#132） ---

    #[test]
    fn diagonal_intersection_rectangle_is_geometric_center() {
        let tl = (0.0, 0.0);
        let tr = (100.0, 0.0);
        let bl = (0.0, 80.0);
        let br = (100.0, 80.0);
        let (ix, iy) = diagonal_intersection(tl, tr, bl, br).expect("交点が求まるべき");
        assert!((ix - 50.0).abs() < 1e-9, "ix={ix}");
        assert!((iy - 40.0).abs() < 1e-9, "iy={iy}");
    }

    #[test]
    fn diagonal_intersection_parallel_diagonals_returns_none() {
        // TL-BR と TR-BL が同じ傾き（10,10）で平行 → 交点なし
        let tl = (0.0, 0.0);
        let br = (10.0, 10.0);
        let tr = (0.0, 5.0);
        let bl = (10.0, 15.0);
        assert!(diagonal_intersection(tl, tr, bl, br).is_none());
    }

    // --- combo_score（#132） ---

    #[test]
    fn combo_score_none_hint_correct_quad_is_minimal() {
        // center_hint=None: アスペクト・対辺比が期待値から逸脱したデコイより
        // テンプレート通りの正クアッドのスコアが小さい（アスペクトが崩れる分デコイが不利）。
        let template = template_quad();
        let mut decoy = template.clone();
        decoy[1].cx += 500.0; // TR を大きく外側にずらして上辺だけ広げる（アスペクト崩壊）
        let s_template = combo_score(&template, None);
        let s_decoy = combo_score(&decoy, None);
        assert!(
            s_template < s_decoy,
            "正クアッドのスコアが最小であるべき: template={s_template} decoy={s_decoy}"
        );
    }

    #[test]
    fn combo_score_center_hint_discriminates_translated_decoy() {
        // aspect/side は平行移動不変なので同値になるデコイ（テンプレート全体を平行移動しただけ）を、
        // 中心マーカー整合項のみで正クアッドより不利にできることを確認する。
        let template = template_quad();
        let (cx, cy) = {
            let tl = (template[0].cx, template[0].cy);
            let tr = (template[1].cx, template[1].cy);
            let bl = (template[2].cx, template[2].cy);
            let br = (template[3].cx, template[3].cy);
            diagonal_intersection(tl, tr, bl, br).unwrap()
        };
        let center = mk(cx, cy);

        let shifted: [DetectedMarker; 4] = [
            mk(template[0].cx + 50.0, template[0].cy + 50.0),
            mk(template[1].cx + 50.0, template[1].cy + 50.0),
            mk(template[2].cx + 50.0, template[2].cy + 50.0),
            mk(template[3].cx + 50.0, template[3].cy + 50.0),
        ];

        let s_template = combo_score(&template, Some(&center));
        let s_shifted = combo_score(&shifted, Some(&center));
        assert!(
            s_template < s_shifted,
            "中心整合のとれた正クアッドが平行移動デコイより低スコアであるべき: template={s_template} shifted={s_shifted}"
        );

        // center_hint なしでは aspect/side が同一なので事実上タイになる（縮退は仕様）
        let s_template_none = combo_score(&template, None);
        let s_shifted_none = combo_score(&shifted, None);
        assert!(
            (s_template_none - s_shifted_none).abs() < 1e-9,
            "center_hint=None では平行移動デコイと同点になる: template={s_template_none} shifted={s_shifted_none}"
        );
    }

    #[test]
    fn combo_score_none_hint_symmetric_decoys_tie_is_pinned_behavior() {
        // #132仕様のピン留め: center_hint=None のとき、aspect/side が同一な複数の
        // デコイ（テンプレートを異なる方向に平行移動しただけ）は同点になる（縮退は仕様であり
        // バグではない）。この挙動を固定する。
        let template = template_quad();
        let decoy_a: [DetectedMarker; 4] = [
            mk(template[0].cx + 30.0, template[0].cy + 40.0),
            mk(template[1].cx + 30.0, template[1].cy + 40.0),
            mk(template[2].cx + 30.0, template[2].cy + 40.0),
            mk(template[3].cx + 30.0, template[3].cy + 40.0),
        ];
        let decoy_b: [DetectedMarker; 4] = [
            mk(template[0].cx - 30.0, template[0].cy - 40.0),
            mk(template[1].cx - 30.0, template[1].cy - 40.0),
            mk(template[2].cx - 30.0, template[2].cy - 40.0),
            mk(template[3].cx - 30.0, template[3].cy - 40.0),
        ];
        let s_a = combo_score(&decoy_a, None);
        let s_b = combo_score(&decoy_b, None);
        assert!(
            (s_a - s_b).abs() < 1e-9,
            "center_hint=None では平行移動方向によらず同点: a={s_a} b={s_b}"
        );
    }

    // --- annulus_white_ratio（#132） ---

    #[test]
    fn annulus_white_ratio_radius_boundary_inclusion() {
        // 半径5.0の格子点シェル（dist==5.0ちょうどの12点: (0,±5)(±5,0)(±3,±4)(±4,±3)）を
        // 白にし、他は全て黒にした20x20画像で、内側/外側それぞれの境界inclusive性を確認する。
        let mut img = GrayImage::from_pixel(20, 20, Luma([0u8]));
        let center = (10i32, 10i32);
        let shell: [(i32, i32); 12] = [
            (0, 5),
            (0, -5),
            (5, 0),
            (-5, 0),
            (3, 4),
            (3, -4),
            (-3, 4),
            (-3, -4),
            (4, 3),
            (4, -3),
            (-4, 3),
            (-4, -3),
        ];
        for (dx, dy) in shell {
            img.put_pixel(
                (center.0 + dx) as u32,
                (center.1 + dy) as u32,
                Luma([255u8]),
            );
        }
        let (cx, cy) = (10.0, 10.0);

        // 内側境界（outer_r=5.001 固定で dist=5.0 のシェル以外を窓の外に追い出す）:
        // dist=5.0 は inner_r=4.999/5.0 では含まれ（not < inner_r）、5.001 では除外される
        assert!(
            (annulus_white_ratio(&img, cx, cy, 4.999, 5.001) - 1.0).abs() < 1e-9,
            "inner_r=4.999: シェルは含まれ全白になるべき"
        );
        assert!(
            (annulus_white_ratio(&img, cx, cy, 5.0, 5.001) - 1.0).abs() < 1e-9,
            "inner_r=5.0 ちょうどは inclusive でシェルを含むべき"
        );
        assert_eq!(
            annulus_white_ratio(&img, cx, cy, 5.001, 5.001),
            0.0,
            "inner_r=5.001 はシェルを除外し total=0 で 0.0 を返すべき"
        );

        // 外側境界（inner_r=4.999 固定で dist=5.0 のシェル以外を窓の外に追い出す）:
        // dist=5.0 は outer_r=5.0/5.001 では含まれ（not > outer_r）、4.999 では除外される
        assert_eq!(
            annulus_white_ratio(&img, cx, cy, 4.999, 4.999),
            0.0,
            "outer_r=4.999 はシェルを除外し total=0 で 0.0 を返すべき"
        );
        assert!(
            (annulus_white_ratio(&img, cx, cy, 4.999, 5.0) - 1.0).abs() < 1e-9,
            "outer_r=5.0 ちょうどは inclusive でシェルを含むべき"
        );
        assert!(
            (annulus_white_ratio(&img, cx, cy, 4.999, 5.001) - 1.0).abs() < 1e-9,
            "outer_r=5.001 はシェルを含むべき"
        );
    }

    #[test]
    fn annulus_white_ratio_image_edge_overflow_does_not_break_calculation() {
        // 中心を画像左上コーナー(0,0)に置き、環状領域の大部分が画像外にはみ出しても
        // total が減るだけでパニックせず、範囲内画素だけで比率が計算されることを確認する。
        let img = GrayImage::from_pixel(50, 50, Luma([255u8]));
        let ratio = annulus_white_ratio(&img, 0.0, 0.0, 5.0, 10.0);
        assert!(
            (0.0..=1.0).contains(&ratio),
            "画像端はみ出しでも比率は[0,1]に収まるべき: ratio={ratio}"
        );
        assert_eq!(ratio, 1.0, "画像内は全て白なので比率は1.0のはず");
    }

    // ══════════════════════════════════════════════════════════════
    // detect_markers 統合テスト（#132）
    // ══════════════════════════════════════════════════════════════
    //
    // 合成 GrayImage を直接構築して detect_markers を通しで検証する。
    // binary と gray は同じ0/255バッファを共有する（gray はサブピクセル精緻化にのみ
    // 使われ、0/255の2値でも refine が原点回帰するだけで結果は破綻しない）。
    // マーカー実寸は layout::mm_to_px(MARKER_SIZE) から実行時に取得し、ハードコードしない。

    fn white_image(w: u32, h: u32) -> GrayImage {
        GrayImage::from_pixel(w, h, Luma([255u8]))
    }

    fn draw_filled_circle(img: &mut GrayImage, cx: f64, cy: f64, radius: f64, value: u8) {
        let icx = cx.round() as i32;
        let icy = cy.round() as i32;
        let r = radius.ceil() as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if ((dx * dx + dy * dy) as f64).sqrt() > radius {
                    continue;
                }
                let px = icx + dx;
                let py = icy + dy;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, Luma([value]));
                }
            }
        }
    }

    fn draw_filled_rect(img: &mut GrayImage, x0: f64, y0: f64, w: f64, h: f64, value: u8) {
        let ix0 = x0.round() as i32;
        let iy0 = y0.round() as i32;
        let iw = w.round() as i32;
        let ih = h.round() as i32;
        for dy in 0..ih {
            for dx in 0..iw {
                let px = ix0 + dx;
                let py = iy0 + dy;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, Luma([value]));
                }
            }
        }
    }

    /// 内側は塗らない円環（外周の annulus 領域だけを塗る）。木目相当の「暗色に囲まれる」
    /// 状況を簡略化して再現する（紙白ゲートは白比率しか見ないため、リアルな木目模様の
    /// 描画は不要。#132 の裁定通り、テストの意図に必要な最小限の合成にとどめる）。
    fn draw_dark_ring(img: &mut GrayImage, cx: f64, cy: f64, inner_r: f64, outer_r: f64) {
        let icx = cx.round() as i32;
        let icy = cy.round() as i32;
        let r = outer_r.ceil() as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                let dist = ((dx * dx + dy * dy) as f64).sqrt();
                if dist < inner_r || dist > outer_r {
                    continue;
                }
                let px = icx + dx;
                let py = icy + dy;
                if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                    img.put_pixel(px as u32, py as u32, Luma([0u8]));
                }
            }
        }
    }

    fn assert_marker_near(m: &DetectedMarker, expected: (f64, f64), tol: f64, label: &str) {
        let d = ((m.cx - expected.0).powi(2) + (m.cy - expected.1).powi(2)).sqrt();
        assert!(
            d <= tol,
            "{label}: got=({:.1},{:.1}) expected≈({:.1},{:.1}) dist={d:.1} tol={tol}",
            m.cx,
            m.cy,
            expected.0,
            expected.1
        );
    }

    /// 1600x2000 の合成キャンバスに、4隅がそれぞれの探索領域内に収まる基準クアッドを描く。
    /// 各マーカーは正円（半径=marker_px/2）。呼び出し側が任意にノイズ・デコイを追加できるよう
    /// 画像と4隅座標を返す（座標順は [TL, TR, BL, BR]）。
    fn baseline_scene() -> (GrayImage, [(f64, f64); 4]) {
        let (w, h) = (1600u32, 2000u32);
        let corners: [(f64, f64); 4] = [
            (150.0, 150.0),
            (1450.0, 150.0),
            (150.0, 1850.0),
            (1450.0, 1850.0),
        ];
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);
        for &(cx, cy) in &corners {
            draw_filled_circle(&mut img, cx, cy, marker_px / 2.0, 0);
        }
        (img, corners)
    }

    #[test]
    fn detect_markers_clean_baseline_with_center_hint() {
        // 観点20: ノイズなし4隅マーカーのみの正常系ベースライン
        let (img, corners) = baseline_scene();
        let center_hint = mk(
            (corners[0].0 + corners[3].0) / 2.0,
            (corners[0].1 + corners[3].1) / 2.0,
        );
        let markers = detect_markers(&img, &img, Some(&center_hint))
            .expect("ノイズなしベースラインは検出できるべき");
        assert_marker_near(&markers[0], corners[0], 3.0, "TL");
        assert_marker_near(&markers[1], corners[1], 3.0, "TR");
        assert_marker_near(&markers[2], corners[2], 3.0, "BL");
        assert_marker_near(&markers[3], corners[3], 3.0, "BR");
    }

    #[test]
    fn detect_markers_none_hint_still_adopts_real_markers() {
        // 観点21: center_hint=None でも正マーカーが採用される（フォールバック経路）
        let (img, corners) = baseline_scene();
        let markers = detect_markers(&img, &img, None).expect("center_hint なしでも検出できるべき");
        assert_marker_near(&markers[0], corners[0], 3.0, "TL");
        assert_marker_near(&markers[1], corners[1], 3.0, "TR");
        assert_marker_near(&markers[2], corners[2], 3.0, "BL");
        assert_marker_near(&markers[3], corners[3], 3.0, "BR");
    }

    #[test]
    fn detect_markers_dense_noise_cluster_deduped_reaches_distant_real_marker() {
        // 観点14: コーナー付近の密集ノイズクラスタ（1つの distinct クラスタとして統合され
        // shape ゲートで棄却される）を超えて、遠い実マーカーに到達・採用することを確認する。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);

        // TopLeft: 実コーナー(0,0)近くに5個の小ブロブを並べる（相互に merge_radius 内）。
        // 統合後の bbox は横長（aspect≫1.667）になり shape ゲートで棄却される。
        for i in 0..5 {
            let x0 = 20.0 + i as f64 * 15.0;
            draw_filled_rect(&mut img, x0, 20.0, 6.0, 6.0, 0);
        }
        let tl_real = (300.0, 380.0);
        draw_filled_circle(&mut img, tl_real.0, tl_real.1, marker_px / 2.0, 0);

        let tr = (1450.0, 150.0);
        let bl = (150.0, 1850.0);
        let br = (1450.0, 1850.0);
        draw_filled_circle(&mut img, tr.0, tr.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, bl.0, bl.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, br.0, br.1, marker_px / 2.0, 0);

        let markers = detect_markers(&img, &img, None)
            .expect("ノイズクラスタを超えて実マーカーに到達・採用できるべき");
        assert_marker_near(
            &markers[0],
            tl_real,
            3.0,
            "TL（ノイズを避けて実マーカーを採用）",
        );
        assert_marker_near(&markers[1], tr, 3.0, "TR");
        assert_marker_near(&markers[2], bl, 3.0, "BL");
        assert_marker_near(&markers[3], br, 3.0, "BR");
    }

    #[test]
    fn detect_markers_one_corner_all_candidates_gated_out_errs() {
        // 観点22: 1隅（TopLeft）に形状ゲートを通らないブロブしかない
        // → 「候補{tried}件、形状/紙白検証を通過した候補なし」Err になる。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);
        // aspect=0.5: ブロブレベルの事前フィルタ(0.35〜3.0)は通過するが、
        // validate_marker_shape のアスペクト帯(0.6〜1.667)には収まらない。
        draw_filled_rect(&mut img, 130.0, 110.0, 40.0, 80.0, 0);

        draw_filled_circle(&mut img, 1450.0, 150.0, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, 150.0, 1850.0, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, 1450.0, 1850.0, marker_px / 2.0, 0);

        let err = detect_markers(&img, &img, None).unwrap_err();
        assert!(
            err.contains("TopLeft")
                && err.contains("候補")
                && err.contains("形状/紙白検証を通過した候補なし"),
            "err={err}"
        );
    }

    #[test]
    fn detect_markers_all_combos_rejected_by_quad_gate_errs() {
        // 観点23: 各隅は単独では形状/紙白ゲートを通過するが、TopLeft の位置がテンプレートの
        // アスペクトから大きく外れるため、唯一の組み合わせがクアッドゲートで棄却される
        // （「配置が不正」系 Err）。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);

        let tl = (60.0, 440.0); // TopLeft領域内の極端な位置（アスペクトを崩す）
        let tr = (1450.0, 150.0);
        let bl = (150.0, 1850.0);
        let br = (1450.0, 1850.0);
        for &(cx, cy) in &[tl, tr, bl, br] {
            draw_filled_circle(&mut img, cx, cy, marker_px / 2.0, 0);
        }

        let err = detect_markers(&img, &img, None).unwrap_err();
        assert!(
            err.contains("マーカー") && err.contains("配置が不正"),
            "err={err}"
        );
    }

    #[test]
    fn detect_markers_paper_white_gate_rejects_decoy_only() {
        // 観点17: 白背景に囲まれた正候補 vs 暗色リングに囲まれた同形デコイ。
        // デコイはコーナーに近く先に試行されるが紙白ゲートで棄却され、
        // 遠い正候補（白背景）が採用される。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let annulus_inner = (marker_px / 2.0) * ANNULUS_INNER_RATIO;
        let annulus_outer = (marker_px / 2.0) * ANNULUS_OUTER_RATIO;
        let mut img = white_image(w, h);

        // デコイ: コーナーに近い(90,90)。marker_px相当の円＋その外側に紙白ゲートを
        // 破る暗色リング（annulus範囲を覆う。円との間に白い隙間を残し4連結で結合させない）。
        let decoy = (90.0, 90.0);
        draw_filled_circle(&mut img, decoy.0, decoy.1, marker_px / 2.0, 0);
        draw_dark_ring(
            &mut img,
            decoy.0,
            decoy.1,
            annulus_inner,
            annulus_outer + 2.0,
        );

        // 正候補: デコイから十分離れた白背景の中（暗色リングの影響が及ばない距離）。
        let real = (300.0, 380.0);
        draw_filled_circle(&mut img, real.0, real.1, marker_px / 2.0, 0);

        let tr = (1450.0, 150.0);
        let bl = (150.0, 1850.0);
        let br = (1450.0, 1850.0);
        draw_filled_circle(&mut img, tr.0, tr.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, bl.0, bl.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, br.0, br.1, marker_px / 2.0, 0);

        let markers =
            detect_markers(&img, &img, None).expect("紙白ゲートでデコイのみ棄却され検出できるべき");
        assert_marker_near(
            &markers[0],
            real,
            3.0,
            "TL（デコイでなく正候補が採用される）",
        );
    }

    #[test]
    fn detect_markers_heading_and_gray_bar_blobs_do_not_block_real_marker() {
        // 観点18: 見出し文字ブロブ（実測110x27, aspect=4.07）とグレーバー黒ステップ
        // （実測5mm×24.4mm, aspect=0.205）相当のブロブが実マーカー付近にあっても、
        // どちらもブロブレベルの事前フィルタ（aspect 0.35〜3.0）で除外され、
        // 実マーカーの検出を妨げないことを確認する。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);

        let real = (200.0, 200.0);
        draw_filled_circle(&mut img, real.0, real.1, marker_px / 2.0, 0);

        // グレーバー黒ステップ相当（aspect=59/288≈0.205 < 0.35 で事前フィルタ除外）
        draw_filled_rect(&mut img, 300.0, 50.0, 59.0, 288.0, 0);
        // 見出し文字列相当（aspect=110/27≈4.07 > 3.0 で事前フィルタ除外）
        draw_filled_rect(&mut img, 20.0, 430.0, 110.0, 27.0, 0);

        let tr = (1450.0, 150.0);
        let bl = (150.0, 1850.0);
        let br = (1450.0, 1850.0);
        draw_filled_circle(&mut img, tr.0, tr.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, bl.0, bl.1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, br.0, br.1, marker_px / 2.0, 0);

        let markers = detect_markers(&img, &img, None)
            .expect("見出し/グレーバー相当ブロブに妨げられず検出できるべき");
        assert_marker_near(&markers[0], real, 3.0, "TL");
    }

    #[test]
    fn detect_markers_decoy_passing_all_gates_rejected_only_by_center_score() {
        // 観点19: 形状・紙白・クアッド幾何ゲートを全て通過するデコイ（テンプレートを
        // 均一平行移動しただけなので aspect/side は正クアッドと同値）が、中心マーカー
        // 整合スコアのみで排除されることを確認する。
        let e = expected_quad_aspect();
        let mean_w = 2400.0f64;
        let mean_h = mean_w / e;
        let real: [(f64, f64); 4] = [
            (300.0, 300.0),
            (300.0 + mean_w, 300.0),
            (300.0, 300.0 + mean_h),
            (300.0 + mean_w, 300.0 + mean_h),
        ];
        let shift = (150.0, 150.0); // 大きさ≈212 > merge_radius(marker_px) なので別クラスタになる
        let decoy: [(f64, f64); 4] = real.map(|(x, y)| (x + shift.0, y + shift.1));

        let w = (real[1].0 + 300.0).round() as u32;
        let h = (real[2].1 + 300.0).round() as u32;
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);
        for &(cx, cy) in real.iter().chain(decoy.iter()) {
            draw_filled_circle(&mut img, cx, cy, marker_px / 2.0, 0);
        }

        let center_hint = mk((real[0].0 + real[3].0) / 2.0, (real[0].1 + real[3].1) / 2.0);
        let markers = detect_markers(&img, &img, Some(&center_hint))
            .expect("正クアッドが中心整合スコアで選ばれ検出できるべき");
        assert_marker_near(
            &markers[0],
            real[0],
            3.0,
            "TL（デコイでなく正クアッドが選ばれる）",
        );
        assert_marker_near(&markers[1], real[1], 3.0, "TR");
        assert_marker_near(&markers[2], real[2], 3.0, "BL");
        assert_marker_near(&markers[3], real[3], 3.0, "BR");
    }

    // ── C-24: エラー文字列契約回帰（#132） ──
    //
    // src/lib/scanner/processor.ts の translateWasmError は detect_markers 由来の
    // エラー文字列を3パターンの部分文字列で分岐する:
    //   1) "マーカーが検出できませんでした"
    //   2) "マーカーの配置が不正" または "四隅マーカーらしい形状が見つかりません"
    // ここでは detect_markers / validate_marker_shape / validate_marker_quad の
    // 全エラーパスの文言が、このいずれかのパターンを含むことを固定する。
    // 司令塔裁定: これはバグではなく契約回帰テストとして固定する（#132セッション記録）。

    fn matches_translate_wasm_error_pattern(err: &str) -> bool {
        err.contains("マーカーが検出できませんでした")
            || err.contains("マーカーの配置が不正")
            || err.contains("四隅マーカーらしい形状が見つかりません")
    }

    #[test]
    fn error_contract_all_detect_markers_paths_match_translate_wasm_error_patterns() {
        let mut errors: Vec<String> = Vec::new();

        // prefilter-empty（フィルタ通過ブロブなし）: 全白画像
        let blank = white_image(400, 500);
        errors.push(detect_markers(&blank, &blank, None).unwrap_err());

        // candidates-empty（形状/紙白検証を通過した候補なし）
        {
            let (w, h) = (1600u32, 2000u32);
            let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
            let mut img = white_image(w, h);
            draw_filled_rect(&mut img, 130.0, 110.0, 40.0, 80.0, 0);
            draw_filled_circle(&mut img, 1450.0, 150.0, marker_px / 2.0, 0);
            draw_filled_circle(&mut img, 150.0, 1850.0, marker_px / 2.0, 0);
            draw_filled_circle(&mut img, 1450.0, 1850.0, marker_px / 2.0, 0);
            errors.push(detect_markers(&img, &img, None).unwrap_err());
        }

        // validate_marker_shape: アスペクト異常 / 大きさ異常
        errors.push(validate_marker_shape("T", 110.0, 27.0, MARKER_PX).unwrap_err());
        errors.push(validate_marker_shape("T", 300.0, 300.0, MARKER_PX).unwrap_err());

        // validate_marker_quad: 辺が短すぎる / 退化 / 対辺比異常 / アスペクト異常
        {
            let q = [
                mk(500.0, 500.0),
                mk(500.0, 500.0),
                mk(500.0, 500.0),
                mk(500.0, 500.0),
            ];
            errors.push(validate_marker_quad(&q).unwrap_err());
        }
        {
            let t = template_quad();
            let q = [t[1].clone(), t[0].clone(), t[2].clone(), t[3].clone()];
            errors.push(validate_marker_quad(&q).unwrap_err());
        }
        {
            // 対辺比異常（w_ratio=59.9/100=0.599<0.6）。全辺は50px以上を保つ。
            let q = [
                mk(-29.95, 0.0),
                mk(29.95, 0.0),
                mk(-50.0, 120.0),
                mk(50.0, 120.0),
            ];
            errors.push(validate_marker_quad(&q).unwrap_err());
        }
        {
            let q = [
                mk(100.0, 100.0),
                mk(2100.0, 100.0),
                mk(100.0, 2100.0),
                mk(2100.0, 2100.0),
            ];
            errors.push(validate_marker_quad(&q).unwrap_err());
        }

        // detect_markers 内の最終フォールバック文字列（現行の制御フローでは
        // per_corner_candidates が全て非空を保証された後にのみ組み合わせ探索に入るため、
        // 実行時に到達するのは事実上困難＝防御的な文言。文字列そのものの契約適合のみ確認する。
        let fallback = "四隅マーカーの配置が不正です（マーカー誤検出の可能性）。四隅のマーカーが隠れず紙全体が写るように撮影してください。";
        errors.push(fallback.to_string());

        for err in &errors {
            assert!(
                matches_translate_wasm_error_pattern(err) && err.contains("マーカー"),
                "translateWasmErrorの3パターンいずれにも一致しない: err={err}"
            );
        }
    }

    // ══════════════════════════════════════════════════════════════
    // detect_markers_near_expected（#132フォローアップ・補正後の局所再検出）
    // ══════════════════════════════════════════════════════════════

    #[test]
    fn detect_markers_near_expected_finds_real_marker_near_window_center() {
        // 期待位置のすぐ近く（残差数px相当）に実マーカーがあれば採用する。
        // 全域探索と違い、期待位置から遠い場所にある別ブロブ（デコイ）は
        // そもそも窓の外なので候補にすらならないことも併せて確認する。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);

        let expected: [(f64, f64); 4] = [
            (150.0, 150.0),
            (1450.0, 150.0),
            (150.0, 1850.0),
            (1450.0, 1850.0),
        ];
        // 実マーカーは期待位置から数px（残差相当）ずれた位置に置く。
        let actual: [(f64, f64); 4] = [
            (152.0, 148.0),
            (1447.0, 153.0),
            (151.0, 1846.0),
            (1453.0, 1852.0),
        ];
        for &(cx, cy) in &actual {
            draw_filled_circle(&mut img, cx, cy, marker_px / 2.0, 0);
        }

        // デコイ: 期待位置から大きく離れた場所（窓の外）に置く。全域探索なら
        // 候補になり得るが、局所探索では窓外なのでそもそも探索されないはず。
        draw_filled_circle(&mut img, 700.0, 1000.0, marker_px / 2.0, 0);

        let search_radius = marker_px * LOCAL_SEARCH_RADIUS_RATIO;
        let markers = detect_markers_near_expected(&img, &img, &expected, search_radius)
            .expect("期待位置近傍の実マーカーを検出できるべき");

        assert_marker_near(&markers[0], actual[0], 3.0, "TL");
        assert_marker_near(&markers[1], actual[1], 3.0, "TR");
        assert_marker_near(&markers[2], actual[2], 3.0, "BL");
        assert_marker_near(&markers[3], actual[3], 3.0, "BR");
    }

    #[test]
    fn detect_markers_near_expected_errs_when_window_has_no_candidate() {
        // TR の期待位置近傍の窓内に一切ブロブが無い（紙面が白いだけ）場合、
        // 全域フォールバックはせず「TopRight マーカーが検出できませんでした」で
        // 即座に Err を返すべき（局所再検出は失敗を隠さず反復中断させる契約）。
        let (w, h) = (1600u32, 2000u32);
        let marker_px = layout::mm_to_px(layout::MARKER_SIZE).round();
        let mut img = white_image(w, h);

        let expected: [(f64, f64); 4] = [
            (150.0, 150.0),
            (1450.0, 150.0),
            (150.0, 1850.0),
            (1450.0, 1850.0),
        ];
        // TL/BL/BR のみ描画し、TR の窓内には何も置かない。
        draw_filled_circle(&mut img, expected[0].0, expected[0].1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, expected[2].0, expected[2].1, marker_px / 2.0, 0);
        draw_filled_circle(&mut img, expected[3].0, expected[3].1, marker_px / 2.0, 0);

        let search_radius = marker_px * LOCAL_SEARCH_RADIUS_RATIO;
        let err = detect_markers_near_expected(&img, &img, &expected, search_radius)
            .expect_err("TR窓内に候補が無ければ Err を返すべき");
        assert!(
            err.contains("TopRight") && err.contains("マーカーが検出できませんでした"),
            "err={err}"
        );
    }
}
