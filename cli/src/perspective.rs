// ページ四隅外挿 + 射影変換（ホモグラフィー行列）
use image::{RgbaImage, Rgba};
use crate::layout;
use crate::marker::DetectedMarker;

/// ページ四隅の座標
#[derive(Debug, Clone)]
pub struct PageCorners {
    pub tl: (f64, f64),
    pub tr: (f64, f64),
    pub bl: (f64, f64),
    pub br: (f64, f64),
}

/// マーカー中心座標からページ四隅を線形外挿する
/// TypeScript extrapolatePageCorners の移植
pub fn extrapolate_page_corners(markers: &[DetectedMarker; 4]) -> PageCorners {
    let ml = &markers[0]; // TL
    let mr = &markers[1]; // TR
    let bl = &markers[2]; // BL
    let br = &markers[3]; // BR

    // マーカー中心のページ内座標（mm）
    let m_left = layout::MARKER_TL.x + layout::MARKER_SIZE / 2.0;   // 3 + 4 = 7
    let m_right = layout::MARKER_TR.x + layout::MARKER_SIZE / 2.0;  // 201 + 4 = 205
    let m_top = layout::MARKER_TL.y + layout::MARKER_SIZE / 2.0;    // 3 + 4 = 7
    let m_bottom = layout::MARKER_BL.y + layout::MARKER_SIZE / 2.0; // 289 + 4 = 293

    let ratio_left = m_left / (m_right - m_left);
    let ratio_right = (layout::PAGE_WIDTH - m_right) / (m_right - m_left);
    let ratio_top = m_top / (m_bottom - m_top);
    let ratio_bottom = (layout::PAGE_HEIGHT - m_bottom) / (m_bottom - m_top);

    println!("  外挿比率: left={ratio_left:.4} right={ratio_right:.4} top={ratio_top:.4} bottom={ratio_bottom:.4}");

    let tl_x = ml.cx - (mr.cx - ml.cx) * ratio_left;
    let tl_y = ml.cy - (bl.cy - ml.cy) * ratio_top;
    let tr_x = mr.cx + (mr.cx - ml.cx) * ratio_right;
    let tr_y = mr.cy - (br.cy - mr.cy) * ratio_top;
    let bl_x = bl.cx - (br.cx - bl.cx) * ratio_left;
    let bl_y = bl.cy + (bl.cy - ml.cy) * ratio_bottom;
    let br_x = br.cx + (br.cx - bl.cx) * ratio_right;
    let br_y = br.cy + (br.cy - mr.cy) * ratio_bottom;

    let corners = PageCorners {
        tl: (tl_x, tl_y),
        tr: (tr_x, tr_y),
        bl: (bl_x, bl_y),
        br: (br_x, br_y),
    };

    println!("  外挿ページ四隅:");
    println!("    TL=({:.1}, {:.1})", corners.tl.0, corners.tl.1);
    println!("    TR=({:.1}, {:.1})", corners.tr.0, corners.tr.1);
    println!("    BL=({:.1}, {:.1})", corners.bl.0, corners.bl.1);
    println!("    BR=({:.1}, {:.1})", corners.br.0, corners.br.1);

    corners
}

/// マーカー4点から直接ホモグラフィーを計算して射影変換
/// 外挿ステップを廃止し、マーカー位置→期待マーカー位置の変換を求める
pub fn homography_warp_from_markers(img: &RgbaImage, markers: &[DetectedMarker; 4]) -> RgbaImage {
    let target_w = layout::image_width();
    let target_h = layout::image_height();

    // 検出されたマーカー中心座標（歪んだ画像上）
    let src = [
        (markers[0].cx, markers[0].cy), // TL
        (markers[1].cx, markers[1].cy), // TR
        (markers[2].cx, markers[2].cy), // BL
        (markers[3].cx, markers[3].cy), // BR
    ];

    // 期待されるマーカー中心座標（補正後画像上、レイアウト定数から計算）
    let marker_defs = [layout::MARKER_TL, layout::MARKER_TR, layout::MARKER_BL, layout::MARKER_BR];
    let dst: [(f64, f64); 4] = std::array::from_fn(|i| {
        let (cx, cy) = layout::marker_center(&marker_defs[i]);
        (layout::mm_to_px(cx), layout::mm_to_px(cy))
    });

    println!("  射影変換(マーカー直接ホモグラフィー): {}x{} → {target_w}x{target_h}", img.width(), img.height());
    for i in 0..4 {
        println!("    マーカー[{i}]: ({:.1},{:.1}) → ({:.1},{:.1})", src[i].0, src[i].1, dst[i].0, dst[i].1);
    }

    // ホモグラフィー行列を求める（dst → src 方向、逆変換用）
    let h = compute_homography(&dst, &src);

    let mut out = RgbaImage::new(target_w, target_h);

    for dy in 0..target_h {
        for dx in 0..target_w {
            let (sx, sy) = apply_homography(&h, dx as f64, dy as f64);
            let pixel = sample_bilinear(img, sx, sy);
            out.put_pixel(dx, dy, pixel);
        }
    }

    out
}

/// 4点対応からホモグラフィー行列 H (3x3) を計算
/// src[i] → dst[i] の変換を求める
/// DLT (Direct Linear Transform) アルゴリズム
fn compute_homography(src: &[(f64, f64); 4], dst: &[(f64, f64); 4]) -> [f64; 9] {
    // 8x9 の行列 A を構成し、Ah=0 を解く
    // 各対応点 (x,y) → (x',y') から2行:
    //   [-x, -y, -1,  0,  0,  0, x*x', y*x', x']
    //   [ 0,  0,  0, -x, -y, -1, x*y', y*y', y']
    let mut a = [[0.0f64; 9]; 8];

    for i in 0..4 {
        let (x, y) = src[i];
        let (xp, yp) = dst[i];

        a[i * 2] = [-x, -y, -1.0, 0.0, 0.0, 0.0, x * xp, y * xp, xp];
        a[i * 2 + 1] = [0.0, 0.0, 0.0, -x, -y, -1.0, x * yp, y * yp, yp];
    }

    // ガウス消去法で8x9の拡大係数行列を解く（h9=1と仮定）
    // 8元連立方程式: h1..h8 を求め、h9=1
    let mut aug = [[0.0f64; 9]; 8];
    for i in 0..8 {
        for j in 0..9 {
            aug[i][j] = a[i][j];
        }
    }

    // h9=1 と仮定して右辺に移す
    // a[i][0..8] * h[0..8] = -a[i][8]
    let mut mat = [[0.0f64; 9]; 8]; // 8x8 + rhs
    for i in 0..8 {
        for j in 0..8 {
            mat[i][j] = aug[i][j];
        }
        mat[i][8] = -aug[i][8]; // 右辺
    }

    // 部分ピボット付きガウス消去
    for col in 0..8 {
        // ピボット選択
        let mut max_row = col;
        let mut max_val = mat[col][col].abs();
        for row in (col + 1)..8 {
            if mat[row][col].abs() > max_val {
                max_val = mat[row][col].abs();
                max_row = row;
            }
        }
        mat.swap(col, max_row);

        let pivot = mat[col][col];
        if pivot.abs() < 1e-12 {
            // 特異行列: 単位行列を返す
            return [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        }

        // 前進消去
        for row in (col + 1)..8 {
            let factor = mat[row][col] / pivot;
            for j in col..9 {
                mat[row][j] -= factor * mat[col][j];
            }
        }
    }

    // 後退代入
    let mut h = [0.0f64; 9];
    h[8] = 1.0;

    for col in (0..8).rev() {
        let mut sum = mat[col][8]; // 右辺
        for j in (col + 1)..8 {
            sum -= mat[col][j] * h[j];
        }
        h[col] = sum / mat[col][col];
    }

    h
}

/// 双線形補間でサンプリング（nearest neighborのエイリアシングを防ぐ）
fn sample_bilinear(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
    let w = img.width() as i64;
    let h = img.height() as i64;

    let x0 = x.floor() as i64;
    let y0 = y.floor() as i64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    // 範囲外は白
    if x0 < 0 || y0 < 0 || x1 >= w || y1 >= h {
        return Rgba([255, 255, 255, 255]);
    }

    let fx = x - x0 as f64;
    let fy = y - y0 as f64;

    let p00 = img.get_pixel(x0 as u32, y0 as u32);
    let p10 = img.get_pixel(x1 as u32, y0 as u32);
    let p01 = img.get_pixel(x0 as u32, y1 as u32);
    let p11 = img.get_pixel(x1 as u32, y1 as u32);

    let mut out = [0u8; 4];
    for c in 0..4 {
        let v = (1.0 - fx) * (1.0 - fy) * p00[c] as f64
              + fx * (1.0 - fy) * p10[c] as f64
              + (1.0 - fx) * fy * p01[c] as f64
              + fx * fy * p11[c] as f64;
        out[c] = v.clamp(0.0, 255.0).round() as u8;
    }
    Rgba(out)
}

/// ホモグラフィー行列を適用: (x, y) → (x', y')
fn apply_homography(h: &[f64; 9], x: f64, y: f64) -> (f64, f64) {
    let w = h[6] * x + h[7] * y + h[8];
    if w.abs() < 1e-12 {
        return (0.0, 0.0);
    }
    let xp = (h[0] * x + h[1] * y + h[2]) / w;
    let yp = (h[3] * x + h[4] * y + h[5]) / w;
    (xp, yp)
}
