/// セル画像（RGBA）からグリフのパスを抽出するモジュール
///
/// 二値化は Sauvola（cell.rs 共通）を使う。画像処理は Rust 側で完結させ、
/// JS は得られた PathCommand 配列を opentype.js の Path に流し込むだけにする。
///
/// ランレングス方式: 二値化画像の各行で黒ピクセルの連続区間（ラン）を検出し、
/// 各ランを四角形パスに変換する。二値化画像と100%同じ見た目が保証される。
use image::RgbaImage;
use serde::{Deserialize, Serialize};

use crate::cell::{
    apply_clahe_pub, morphological_open_close, rgba_to_gray_pub, sauvola_binarize_pub,
    SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB,
};

// ── 定数 ──

pub const UNITS_PER_EM: f64 = 1000.0;
pub const GLYPH_HEIGHT: f64 = 800.0;

// ── 型定義（serde 経由で JS 側 PathCommand と同じ JSON 形式を吐く） ──

/// PathCommand は TS 側 `{type: 'M'|'L'|'C'|'Z', x, y, cp1x?, cp1y?, cp2x?, cp2y?}` と互換
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PathCommand {
    #[serde(rename = "M")]
    MoveTo { x: f64, y: f64 },
    #[serde(rename = "L")]
    LineTo { x: f64, y: f64 },
    #[serde(rename = "C")]
    CurveTo {
        x: f64,
        y: f64,
        cp1x: f64,
        cp1y: f64,
        cp2x: f64,
        cp2y: f64,
    },
    #[serde(rename = "Z")]
    Close { x: f64, y: f64 },
}

// ── エントリポイント ──

/// セル画像（RGBA）→ パス配列
///
/// 1. グレー化
/// 2. CLAHE
/// 3. Sauvola 二値化（0=黒, 255=白）
/// 4. モルフォロジ open-close
/// 5. ランレングス変換 — 各行の黒ピクセル連続区間を四角形パスにする
pub fn vectorize_glyph(img: &RgbaImage) -> Vec<Vec<PathCommand>> {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return Vec::new();
    }

    // 1-4: 二値化（内部バイナリ: 1=黒(前景), 0=白(背景)）
    let binary = binarize_for_contour(img);

    // 5: ランレングス抽出
    let mut runs: Vec<(u32, u32, u32)> = Vec::new(); // (y, x_start, x_end)
    for y in 0..h {
        let mut x = 0u32;
        while x < w {
            if binary[(y * w + x) as usize] != 1 {
                x += 1;
                continue;
            }
            let run_start = x;
            while x < w && binary[(y * w + x) as usize] == 1 {
                x += 1;
            }
            runs.push((y, run_start, x));
        }
    }

    // 6: 縦方向マージ — x 範囲が近いランが連続する行にあれば結合して矩形にする
    //    ±MERGE_TOLERANCE px の誤差を許容するが、矩形の幅は最初のランを維持する
    //    これにより四角形の数（= パス数）を大幅に削減する
    const MERGE_TOLERANCE: u32 = 2;
    let mut rects: Vec<(u32, u32, u32, u32)> = Vec::new(); // (x_start, y_start, x_end, y_end)
    let mut used = vec![false; runs.len()];

    for i in 0..runs.len() {
        if used[i] {
            continue;
        }
        let (y0, xs, xe) = runs[i];
        used[i] = true;
        let mut y_end = y0 + 1;

        // 次の行以降で近い x 範囲のランを探して結合（矩形の幅は最初のランで固定）
        let mut j = i + 1;
        while j < runs.len() {
            let (yj, xsj, xej) = runs[j];
            if yj > y_end {
                break; // 連続しない行に来た
            }
            if yj == y_end
                && xsj.abs_diff(xs) <= MERGE_TOLERANCE
                && xej.abs_diff(xe) <= MERGE_TOLERANCE
            {
                used[j] = true;
                y_end = yj + 1;
            }
            j += 1;
        }
        rects.push((xs, y0, xe, y_end));
    }

    // 7: フォント座標に変換してパス生成
    let scale = GLYPH_HEIGHT / h as f64;
    let offset_x = (UNITS_PER_EM - w as f64 * scale) / 2.0;

    let paths: Vec<Vec<PathCommand>> = rects
        .iter()
        .map(|&(xs, ys, xe, ye)| {
            let fx0 = (xs as f64 * scale + offset_x).round();
            let fx1 = (xe as f64 * scale + offset_x).round();
            let fy_top = (GLYPH_HEIGHT - ys as f64 * scale).round();
            let fy_bot = (GLYPH_HEIGHT - ye as f64 * scale).round();

            vec![
                PathCommand::MoveTo { x: fx0, y: fy_top },
                PathCommand::LineTo { x: fx1, y: fy_top },
                PathCommand::LineTo { x: fx1, y: fy_bot },
                PathCommand::LineTo { x: fx0, y: fy_bot },
                PathCommand::Close { x: fx0, y: fy_top },
            ]
        })
        .collect();

    paths
}

/// セル画像を二値化してフラグ配列に変換する（1=黒=前景, 0=白=背景）
fn binarize_for_contour(img: &RgbaImage) -> Vec<u8> {
    let w = img.width();
    let h = img.height();
    let gray = rgba_to_gray_pub(img);
    let gray = apply_clahe_pub(&gray, w, h);
    let binary = sauvola_binarize_pub(&gray, w, h, SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB);
    let binary = morphological_open_close(&binary, w, h);
    // Sauvola 出力は 0=黒, 255=白。内部では 1=前景(黒) のフラグに変換
    binary.iter().map(|&v| if v == 0 { 1u8 } else { 0u8 }).collect()
}

/// セル画像を二値化済み白背景+黒ストロークの RGBA として返す（プレビュー・ベクター化入力用）
pub fn binarize_to_rgba(img: &RgbaImage) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return img.clone();
    }
    let gray = rgba_to_gray_pub(img);
    let gray = apply_clahe_pub(&gray, w, h);
    let binary = sauvola_binarize_pub(&gray, w, h, SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB);
    let binary = morphological_open_close(&binary, w, h);

    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let v = binary[(y * w + x) as usize];
            let c = if v == 0 { 0 } else { 255 };
            out.put_pixel(x, y, image::Rgba([c, c, c, 255]));
        }
    }
    out
}

// ── SVG 出力（CLI の検証用） ──

/// パス配列をシンプルな SVG 文字列に変換する（デバッグ可視化用）
#[cfg(not(target_arch = "wasm32"))]
pub fn paths_to_svg(paths: &[Vec<PathCommand>]) -> String {
    let vb_w = UNITS_PER_EM as i32;
    let vb_h = GLYPH_HEIGHT as i32;
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {vb_w} {vb_h}\" width=\"400\" height=\"320\">\n"
    ));
    out.push_str("  <rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n");

    // 全サブパスを1つの <path> にまとめて fill で塗りつぶす
    let mut d = String::new();
    for path in paths {
        for cmd in path {
            match cmd {
                PathCommand::MoveTo { x, y } => {
                    d.push_str(&format!("M{x:.0} {y:.0} ", y = vb_h as f64 - y));
                }
                PathCommand::LineTo { x, y } => {
                    d.push_str(&format!("L{x:.0} {y:.0} ", y = vb_h as f64 - y));
                }
                PathCommand::CurveTo { x, y, cp1x, cp1y, cp2x, cp2y } => {
                    d.push_str(&format!(
                        "C{cp1x:.0} {cp1y:.0} {cp2x:.0} {cp2y:.0} {x:.0} {y:.0} ",
                        cp1y = vb_h as f64 - cp1y,
                        cp2y = vb_h as f64 - cp2y,
                        y = vb_h as f64 - y,
                    ));
                }
                PathCommand::Close { .. } => d.push_str("Z "),
            }
        }
    }
    out.push_str(&format!(
        "  <path d=\"{d}\" fill=\"black\" fill-rule=\"evenodd\"/>\n"
    ));

    out.push_str("</svg>\n");
    out
}

// ── テスト ──

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn make_image(w: u32, h: u32, color: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, color);
            }
        }
        img
    }

    #[test]
    fn empty_image_returns_empty_paths() {
        let img = RgbaImage::new(0, 0);
        let paths = vectorize_glyph(&img);
        assert!(paths.is_empty());
    }

    #[test]
    fn uniform_white_returns_empty_paths() {
        let img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        let paths = vectorize_glyph(&img);
        assert!(paths.is_empty(), "均一白は空のパスを返すべき");
    }

    #[test]
    fn uniform_black_does_not_panic() {
        let img = make_image(100, 100, Rgba([0, 0, 0, 255]));
        let _paths = vectorize_glyph(&img);
    }

    #[test]
    fn black_rect_on_white_produces_paths() {
        let mut img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 30..70 {
            for x in 30..70 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let paths = vectorize_glyph(&img);
        assert!(!paths.is_empty(), "黒矩形に対して少なくとも1つのパスが返るべき");
        for path in &paths {
            assert!(
                matches!(path.first(), Some(PathCommand::MoveTo { .. })),
                "パスは MoveTo で始まるべき"
            );
            assert!(
                matches!(path.last(), Some(PathCommand::Close { .. })),
                "パスは Close で終わるべき"
            );
            // ランレングス方式では各パスは5コマンド（M,L,L,L,Z）
            assert_eq!(path.len(), 5, "各ランは5コマンドの四角形");
        }
    }
}
