// 擬似歪み生成（台形補正テスト用）
// スマホ撮影をシミュレート: 回転 + 台形変形 + グレー背景 + 余白

use image::{RgbaImage, Rgba};
use std::path::Path;

/// 画像に擬似歪みを加える
pub fn distort_image(
    input_path: &Path,
    output_path: &Path,
    rotate_deg: f64,
    trapezoid_strength: f64,
    padding: u32,
) -> Result<(), String> {
    let img = image::open(input_path)
        .map_err(|e| format!("画像読み込みエラー: {e}"))?
        .to_rgba8();

    let src_w = img.width() as f64;
    let src_h = img.height() as f64;

    println!("入力画像: {}x{}", img.width(), img.height());
    println!("歪みパラメータ: 回転={rotate_deg}° 台形={trapezoid_strength} 余白={padding}px");

    // 出力キャンバスサイズ（余白込み）
    let out_w = img.width() + padding * 2;
    let out_h = img.height() + padding * 2;

    // グレー背景（机/床をシミュレート）
    let bg_color = Rgba([200, 195, 185, 255]);
    let mut out = RgbaImage::from_pixel(out_w, out_h, bg_color);

    // 元画像の四隅の出力座標を計算
    // 1. 余白の中央に配置
    // 2. 回転を適用
    // 3. 台形変形を適用（上辺を狭くする = カメラが上から斜めに見た感じ）
    let cx = out_w as f64 / 2.0;
    let cy = out_h as f64 / 2.0;

    let rad = rotate_deg * std::f64::consts::PI / 180.0;
    let cos_r = rad.cos();
    let sin_r = rad.sin();

    // 元画像四隅（中心基準）
    let half_w = src_w / 2.0;
    let half_h = src_h / 2.0;
    let corners_local = [
        (-half_w, -half_h), // TL
        ( half_w, -half_h), // TR
        (-half_w,  half_h), // BL
        ( half_w,  half_h), // BR
    ];

    // 回転適用
    let rotated: Vec<(f64, f64)> = corners_local.iter().map(|&(x, y)| {
        (x * cos_r - y * sin_r, x * sin_r + y * cos_r)
    }).collect();

    // 台形変形: 上辺を狭く、下辺を広く
    // TL/TR の x を内側に、BL/BR の x を外側に
    let t = trapezoid_strength;
    let trapezoid_corners = [
        (rotated[0].0 + half_w * t, rotated[0].1 - half_h * t * 0.3), // TL: 右下に
        (rotated[1].0 - half_w * t, rotated[1].1 - half_h * t * 0.3), // TR: 左下に
        (rotated[2].0 - half_w * t * 0.5, rotated[2].1 + half_h * t * 0.2), // BL: 少し左下に
        (rotated[3].0 + half_w * t * 0.5, rotated[3].1 + half_h * t * 0.2), // BR: 少し右下に
    ];

    // 出力座標に変換（中心オフセット適用）
    let dst_corners: Vec<(f64, f64)> = trapezoid_corners.iter().map(|&(x, y)| {
        (cx + x, cy + y)
    }).collect();

    println!("出力四隅:");
    println!("  TL=({:.1}, {:.1})", dst_corners[0].0, dst_corners[0].1);
    println!("  TR=({:.1}, {:.1})", dst_corners[1].0, dst_corners[1].1);
    println!("  BL=({:.1}, {:.1})", dst_corners[2].0, dst_corners[2].1);
    println!("  BR=({:.1}, {:.1})", dst_corners[3].0, dst_corners[3].1);

    // 逆変換: 出力の各ピクセルに対して元画像の座標を算出（双線形マッピング）
    let (tl, tr, bl, br) = (dst_corners[0], dst_corners[1], dst_corners[2], dst_corners[3]);

    for dy in 0..out_h {
        for dx in 0..out_w {
            let ox = dx as f64;
            let oy = dy as f64;

            // 双線形逆マッピング: 出力座標 → 正規化座標 (u, v) → 元画像座標
            // 出力四隅に対する逆マッピングを解く
            if let Some((u, v)) = inverse_bilinear(ox, oy, tl, tr, bl, br) {
                if u >= 0.0 && u <= 1.0 && v >= 0.0 && v <= 1.0 {
                    let sx = (u * (src_w - 1.0)).round() as u32;
                    let sy = (v * (src_h - 1.0)).round() as u32;
                    if sx < img.width() && sy < img.height() {
                        out.put_pixel(dx, dy, *img.get_pixel(sx, sy));
                    }
                }
            }
        }
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("出力ディレクトリ作成エラー: {e}"))?;
    }
    out.save(output_path)
        .map_err(|e| format!("画像保存エラー: {e}"))?;

    println!("歪み画像保存: {} ({}x{})", output_path.display(), out_w, out_h);
    Ok(())
}

/// 双線形逆マッピング: 出力座標(px, py)から正規化座標(u, v)を算出
/// 四隅 tl, tr, bl, br に対して、px = (1-u)(1-v)*tl + u*(1-v)*tr + (1-u)*v*bl + u*v*br
/// を u, v について解く（Newton法）
fn inverse_bilinear(
    px: f64, py: f64,
    tl: (f64, f64), tr: (f64, f64), bl: (f64, f64), br: (f64, f64),
) -> Option<(f64, f64)> {
    let mut u = 0.5;
    let mut v = 0.5;

    for _ in 0..20 {
        // 順方向: (u, v) → (x, y)
        let fx = (1.0 - u) * (1.0 - v) * tl.0 + u * (1.0 - v) * tr.0
               + (1.0 - u) * v * bl.0 + u * v * br.0 - px;
        let fy = (1.0 - u) * (1.0 - v) * tl.1 + u * (1.0 - v) * tr.1
               + (1.0 - u) * v * bl.1 + u * v * br.1 - py;

        if fx.abs() < 0.01 && fy.abs() < 0.01 {
            return Some((u, v));
        }

        // ヤコビアン
        let dfx_du = -(1.0 - v) * tl.0 + (1.0 - v) * tr.0 - v * bl.0 + v * br.0;
        let dfx_dv = -(1.0 - u) * tl.0 - u * tr.0 + (1.0 - u) * bl.0 + u * br.0;
        let dfy_du = -(1.0 - v) * tl.1 + (1.0 - v) * tr.1 - v * bl.1 + v * br.1;
        let dfy_dv = -(1.0 - u) * tl.1 - u * tr.1 + (1.0 - u) * bl.1 + u * br.1;

        let det = dfx_du * dfy_dv - dfx_dv * dfy_du;
        if det.abs() < 1e-10 {
            return None;
        }

        u -= (dfy_dv * fx - dfx_dv * fy) / det;
        v -= (-dfy_du * fx + dfx_du * fy) / det;
    }

    Some((u, v))
}
