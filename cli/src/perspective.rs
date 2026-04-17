// ページ四隅外挿 + 射影変換（ホモグラフィー行列）
use image::{RgbaImage, Rgba};
use crate::layout;
use crate::marker::DetectedMarker;

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

    log!("  射影変換(マーカー直接ホモグラフィー): {}x{} → {target_w}x{target_h}", img.width(), img.height());
    for i in 0..4 {
        log!("    マーカー[{i}]: ({:.1},{:.1}) → ({:.1},{:.1})", src[i].0, src[i].1, dst[i].0, dst[i].1);
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
pub fn sample_bilinear(img: &RgbaImage, x: f64, y: f64) -> Rgba<u8> {
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

/// 補正後画像上のマーカー検出位置 → 期待位置のリファインメントホモグラフィー
/// detected は補正後画像上のマーカー位置。期待位置はレイアウト定数から計算。
/// detected → expected の変換で img を再ワープする。
pub fn homography_refine(img: &RgbaImage, detected: &[DetectedMarker; 4]) -> RgbaImage {
    let target_w = layout::image_width();
    let target_h = layout::image_height();

    // 補正後画像上の検出位置（ずれている）
    let src = [
        (detected[0].cx, detected[0].cy),
        (detected[1].cx, detected[1].cy),
        (detected[2].cx, detected[2].cy),
        (detected[3].cx, detected[3].cy),
    ];

    // 期待されるマーカー中心座標（レイアウト定数から計算）
    let marker_defs = [layout::MARKER_TL, layout::MARKER_TR, layout::MARKER_BL, layout::MARKER_BR];
    let dst: [(f64, f64); 4] = std::array::from_fn(|i| {
        let (cx, cy) = layout::marker_center(&marker_defs[i]);
        (layout::mm_to_px(cx), layout::mm_to_px(cy))
    });

    log!("  リファインメント変換:");
    for i in 0..4 {
        log!("    マーカー[{i}]: ({:.1},{:.1}) → ({:.1},{:.1})", src[i].0, src[i].1, dst[i].0, dst[i].1);
    }

    // ホモグラフィー行列（dst → src 方向、逆変換用）
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

// ── TPS（Thin Plate Spline）ワープ ──
//
// ホモグラフィーは4点で厳密に決まり、平面→平面の射影を表現できるが、
// カメラレンズの樽型／糸巻き型歪みによって紙面の中央が膨らむ／凹むケースは
// どの4点を選んでも補正できない。
//
// TPSは制御点を厳密に通り、曲げエネルギーを最小化する滑らかな2変数スプラインを
// 求める。中心マーカー1点を加えるだけだと中心は合うが上下端が引っ張られて
// ズレるため、4辺中点も「ホモグラフィー後の理想位置（src=dst）」として制御点に
// 含めて9点でフィットする。これで境界はホモグラフィーが効き、内部だけ
// 中心マーカーで引き戻される。
//
// 参考: Bookstein 1989 "Principal Warps: Thin-Plate Splines and the
// Decomposition of Deformations"

/// N点TPSで img を再ワープする。
///
/// src_pts[i] は「補正後画像上の検出位置」、dst_pts[i] は「レイアウト期待位置」。
/// dst 側の各ピクセルに対し、対応する src 側のサンプリング座標を TPS で求めて描く。
pub fn tps_warp(img: &RgbaImage, src_pts: &[(f64, f64)], dst_pts: &[(f64, f64)]) -> RgbaImage {
    assert_eq!(src_pts.len(), dst_pts.len(), "src と dst の点数が異なります");

    let target_w = layout::image_width();
    let target_h = layout::image_height();

    // dst → src を TPS でフィット（x成分・y成分それぞれ独立）
    let coef_x = fit_tps(dst_pts, src_pts, true);
    let coef_y = fit_tps(dst_pts, src_pts, false);

    let mut out = RgbaImage::new(target_w, target_h);
    for dy in 0..target_h {
        for dx in 0..target_w {
            let sx = eval_tps(&coef_x, dst_pts, dx as f64, dy as f64);
            let sy = eval_tps(&coef_y, dst_pts, dx as f64, dy as f64);
            let pixel = sample_bilinear(img, sx, sy);
            out.put_pixel(dx, dy, pixel);
        }
    }
    out
}

/// TPSの基底関数 U(r) = r² ln(r²)。r=0では0とする。
fn tps_u(r2: f64) -> f64 {
    if r2 < 1e-12 { 0.0 } else { r2 * r2.ln() }
}

/// N点TPSの係数 [w_0..w_{N-1}, a_0, a_1, a_2] を求める。
/// `use_x` が true のとき target[i].0（x成分）を、false のとき y成分をフィットする。
fn fit_tps(ctrl: &[(f64, f64)], target: &[(f64, f64)], use_x: bool) -> Vec<f64> {
    let n = ctrl.len();
    let m = n + 3;
    let mut l = vec![vec![0.0f64; m]; m];

    // K: l[i][j] = U(|p_i - p_j|²)
    for i in 0..n {
        for j in 0..n {
            let dx = ctrl[i].0 - ctrl[j].0;
            let dy = ctrl[i].1 - ctrl[j].1;
            l[i][j] = tps_u(dx * dx + dy * dy);
        }
    }
    // P (n x 3) 右上と P^T (3 x n) 左下
    for i in 0..n {
        l[i][n] = 1.0;
        l[i][n + 1] = ctrl[i].0;
        l[i][n + 2] = ctrl[i].1;
        l[n][i] = 1.0;
        l[n + 1][i] = ctrl[i].0;
        l[n + 2][i] = ctrl[i].1;
    }

    let mut rhs = vec![0.0f64; m];
    for i in 0..n {
        rhs[i] = if use_x { target[i].0 } else { target[i].1 };
    }

    solve_linear(l, rhs)
}

/// TPSを評価: f(p) = a_0 + a_1 x + a_2 y + Σ w_i U(|p - p_i|²)
fn eval_tps(coef: &[f64], ctrl: &[(f64, f64)], x: f64, y: f64) -> f64 {
    let n = ctrl.len();
    let mut r = coef[n] + coef[n + 1] * x + coef[n + 2] * y;
    for i in 0..n {
        let dx = x - ctrl[i].0;
        let dy = y - ctrl[i].1;
        r += coef[i] * tps_u(dx * dx + dy * dy);
    }
    r
}

/// 部分ピボット付きガウス消去。特異な場合は零ベクトルを返す。
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Vec<f64> {
    let n = a.len();
    for col in 0..n {
        let mut max_row = col;
        let mut max_val = a[col][col].abs();
        for row in (col + 1)..n {
            if a[row][col].abs() > max_val {
                max_val = a[row][col].abs();
                max_row = row;
            }
        }
        a.swap(col, max_row);
        b.swap(col, max_row);
        let pivot = a[col][col];
        if pivot.abs() < 1e-12 {
            return vec![0.0; n];
        }
        for row in (col + 1)..n {
            let factor = a[row][col] / pivot;
            for j in col..n {
                a[row][j] -= factor * a[col][j];
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0f64; n];
    for col in (0..n).rev() {
        let mut sum = b[col];
        for j in (col + 1)..n {
            sum -= a[col][j] * x[j];
        }
        x[col] = sum / a[col][col];
    }
    x
}

// ── テスト ──

#[cfg(test)]
mod tests {
    use super::*;

    /// 恒等 TPS: src == dst の9点でフィットすると、任意の点で f(x,y) = x / y を返すはず
    /// （= 重み w_i がすべて 0、アフィン部が identity）
    #[test]
    fn tps_identity_returns_input() {
        let pts: Vec<(f64, f64)> = vec![
            (82.7, 82.7),   (2421.3, 82.7),  (82.7, 3436.0), (2421.3, 3436.0),
            (1252.0, 82.7), (1252.0, 3436.0),(82.7, 1759.4), (2421.3, 1759.4),
            (1252.0, 1771.7),
        ];
        let coef_x = fit_tps(&pts, &pts, true);
        let coef_y = fit_tps(&pts, &pts, false);

        // 制御点・非制御点いずれでも src = dst になるはず
        for &(tx, ty) in &[(100.0, 200.0), (1500.0, 800.0), (82.7, 82.7), (1252.0, 1771.7)] {
            let sx = eval_tps(&coef_x, &pts, tx, ty);
            let sy = eval_tps(&coef_y, &pts, tx, ty);
            assert!((sx - tx).abs() < 1e-4, "identity TPS: x {tx} -> {sx}");
            assert!((sy - ty).abs() < 1e-4, "identity TPS: y {ty} -> {sy}");
        }
    }

    /// TPS は制御点で厳密に target 値を取るべき（補間性）
    #[test]
    fn tps_passes_through_control_points() {
        let dst: Vec<(f64, f64)> = vec![
            (0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0), (50.0, 50.0),
        ];
        let src: Vec<(f64, f64)> = vec![
            (0.0, 0.0), (100.0, 0.0), (0.0, 100.0), (100.0, 100.0), (40.0, 60.0), // 中心だけズラす
        ];
        let coef_x = fit_tps(&dst, &src, true);
        let coef_y = fit_tps(&dst, &src, false);

        for i in 0..dst.len() {
            let sx = eval_tps(&coef_x, &dst, dst[i].0, dst[i].1);
            let sy = eval_tps(&coef_y, &dst, dst[i].0, dst[i].1);
            assert!(
                (sx - src[i].0).abs() < 1e-6 && (sy - src[i].1).abs() < 1e-6,
                "TPS should interpolate at control point {i}: got ({sx},{sy}), want ({},{})",
                src[i].0, src[i].1
            );
        }
    }

    /// ガウス消去ソルバの動作確認（2x2 の閉形式ケース）
    #[test]
    fn solve_linear_small_case() {
        // | 2 1 | |x|   |5|      x=2, y=1
        // | 1 3 | |y| = |5|
        let a = vec![vec![2.0, 1.0], vec![1.0, 3.0]];
        let b = vec![5.0, 5.0];
        let x = solve_linear(a, b);
        assert!((x[0] - 2.0).abs() < 1e-9);
        assert!((x[1] - 1.0).abs() < 1e-9);
    }

    /// U(r²) は r=0 で 0 を返し、r>0 で単調増加
    #[test]
    fn tps_u_basis_properties() {
        assert_eq!(tps_u(0.0), 0.0);
        assert_eq!(tps_u(1e-13), 0.0); // 数値安定化のしきい値内
        assert!(tps_u(4.0) < tps_u(9.0));
        assert!(tps_u(9.0) < tps_u(16.0));
    }
}
