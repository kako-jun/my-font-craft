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
    apply_cell_quality_gate, apply_clahe_pub, compensate_ink_bleed, morphological_open_close,
    rgba_to_gray_pub, sauvola_binarize_pub, CellQuality, SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB,
};
use crate::layout;

// ── 定数 ──

pub const UNITS_PER_EM: f64 = 1000.0;
pub const GLYPH_HEIGHT: f64 = 800.0;
/// bbox 正規化（opt-in 救済、#111 で既定経路から除外）の目標サイズ（長辺、units）。
/// 日本語フォントの慣例（ideographic body ≒ em の 75%）に合わせた 750。
/// 既定経路はセル→em 固定変換（下記 vectorize_binary）であり、この定数は
/// vectorize_binary_bbox_fit でのみ使う。
pub const EM_FIT_SIZE: f64 = 750.0;

/// em 座標系の units / mm。内枠（INNER_SIZE = 10mm）を em-square（1000 units）に写す。
pub fn em_units_per_mm() -> f64 {
    UNITS_PER_EM / layout::INNER_SIZE
}

/// セル crop 内での内枠左端の位置（mm）。crop はセル外枠から
/// CELL_CROP_MARGIN(1.5mm) 内側、内枠はセル外枠から (15-10)/2 = 2.5mm 内側。
fn inner_left_in_crop_mm() -> f64 {
    (layout::CELL_SIZE - layout::INNER_SIZE) / 2.0 - layout::CELL_CROP_MARGIN
}

/// セル crop 全域が写る em 座標範囲 (x_min, y_min, x_max, y_max)。
/// SVG プレビューの viewBox 用（descender 領域 y<0 を切らずに表示する）。
/// 既定レイアウトでは (-100, -220, 1100, 980)。
pub fn crop_em_bounds() -> (f64, f64, f64, f64) {
    let em = em_units_per_mm();
    let inner_left = inner_left_in_crop_mm();
    let inner_bottom = inner_left + layout::INNER_SIZE;
    let x_min = -inner_left * em;
    let x_max = (layout::CELL_CROP_SIZE - inner_left) * em;
    let y_min = layout::EMBOX_BOTTOM_Y + (inner_bottom - layout::CELL_CROP_SIZE) * em;
    let y_max = layout::EMBOX_BOTTOM_Y + inner_bottom * em;
    (x_min, y_min, x_max, y_max)
}

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
/// 1. グレー化 + CLAHE + Sauvola 二値化 + モルフォロジ open-close + 品質ゲート（#110）
/// 2. 2x nearest-neighbor アップスケール（ドット細粒化）
/// 3. ランレングス抽出（各行の黒ピクセル連続区間を検出）
/// 4. 縦方向マージ（±2px 許容で隣接行のランを矩形に結合）
/// 5. フォント座標変換（各矩形を M→L→L→L→Z の四角形パスに変換）
pub fn vectorize_glyph(img: &RgbaImage) -> Vec<Vec<PathCommand>> {
    let (binary, _quality) = binarize_with_quality(img);
    vectorize_binary(&binary, img.width(), img.height())
}

/// 二値化済みセル（Sauvola 形式: 0=黒/255=白、品質ゲート適用済み）→ パス配列
///
/// pipeline 側で二値化を1回だけ行い、プレビュー RGBA とベクター化の入力を
/// 完全に一致させるための分割エントリポイント。
///
/// 座標変換（#111）: **セル矩形→em の固定アフィン変換**。
/// 入力画像は「セル外枠から CELL_CROP_MARGIN(1.5mm) 内側を crop した
/// CELL_CROP_SIZE(12mm) 四方」である前提（cell.rs extract_cell_image_raw）。
/// 内枠（10mm、書く領域）を em-square [0,1000] × [EMBOX_BOTTOM_Y, EMBOX_BOTTOM_Y+1000]
/// = [-120, 880] に写す（1mm = 100 units）。書き手がセル内のどこに・どの大きさで
/// 書いたかがそのままフォントに出る。旧 bbox 正規化は vectorize_binary_bbox_fit
/// （opt-in 救済、既定 OFF）に格下げ。
pub fn vectorize_binary(binary_sauvola: &[u8], w: u32, h: u32) -> Vec<Vec<PathCommand>> {
    let Some((rects, uw, uh)) = extract_rects(binary_sauvola, w, h) else {
        return Vec::new();
    };

    let em = em_units_per_mm();
    // 幅と高さは同じ mm（正方 crop）だが mm→px の丸めで px 数が異なりうるため軸ごとに算出
    let px_per_mm_x = uw as f64 / layout::CELL_CROP_SIZE;
    let px_per_mm_y = uh as f64 / layout::CELL_CROP_SIZE;
    let inner_left_mm = inner_left_in_crop_mm(); // crop 内での内枠左端 = 1.0mm
    let inner_bottom_mm = inner_left_mm + layout::INNER_SIZE; // crop 内での内枠下端 = 11.0mm

    // 画像座標(px, Y下向き) → em 座標(units, Y上向き)
    let fx = |px: f64| ((px / px_per_mm_x - inner_left_mm) * em).round();
    let fy =
        |py: f64| (layout::EMBOX_BOTTOM_Y + (inner_bottom_mm - py / px_per_mm_y) * em).round();

    rects_to_paths(&rects, fx, fy)
}

/// bbox 正規化（旧 #53 方式）: 黒ピクセルの bbox を EM_FIT_SIZE(750) に拡大して
/// em 中央に配置する。
///
/// #111 で既定経路から外した opt-in の救済。書いた位置・大きさを捨てるため、
/// 句読点が行中央に浮く・小書きかなが等倍化する・descender が失われる副作用がある。
/// 「セルに対して明らかに小さすぎる字を後から拡大したい」ケース専用で、既定 OFF
/// （現在プロダクション経路からの呼び出しなし。判断ロジックの保存とテストのために残す）。
pub fn vectorize_binary_bbox_fit(binary_sauvola: &[u8], w: u32, h: u32) -> Vec<Vec<PathCommand>> {
    let Some((rects, _uw, _uh)) = extract_rects(binary_sauvola, w, h) else {
        return Vec::new();
    };

    // タイトバウンディングボックス検出（#53 Phase 1）
    let bx_min = rects.iter().map(|r| r.0).min().unwrap() as f64;
    let by_min = rects.iter().map(|r| r.1).min().unwrap() as f64;
    let bx_max = rects.iter().map(|r| r.2).max().unwrap() as f64;
    let by_max = rects.iter().map(|r| r.3).max().unwrap() as f64;
    let bbox_w = bx_max - bx_min;
    let bbox_h = by_max - by_min;
    // rects 非空なら runlength 抽出の不変条件から bbox_w >= 1, bbox_h >= 1 のはずだが、
    // ゼロ除算と負スケールを防ぐ念のためのガード。
    if bbox_w < 1.0 || bbox_h < 1.0 {
        return Vec::new();
    }

    // em-square フィット（#53 Phase 2）
    // アスペクト比を保ったまま bbox 長辺が EM_FIT_SIZE になるようスケール。em-square 中央に配置。
    let scale = EM_FIT_SIZE / bbox_w.max(bbox_h);
    let final_w = bbox_w * scale;
    let final_h = bbox_h * scale;
    let offset_x = (UNITS_PER_EM - final_w) / 2.0;
    let offset_y = (GLYPH_HEIGHT - final_h) / 2.0;

    let fx = |px: f64| ((px - bx_min) * scale + offset_x).round();
    let fy = |py: f64| (offset_y + final_h - (py - by_min) * scale).round();

    rects_to_paths(&rects, fx, fy)
}

/// 矩形群を M→L→L→L→Z の四角形パスに変換する。
/// fx/fy は画像座標(px, Y下向き)→フォント座標(units, Y上向き)の写像。
fn rects_to_paths(
    rects: &[(u32, u32, u32, u32)],
    fx: impl Fn(f64) -> f64,
    fy: impl Fn(f64) -> f64,
) -> Vec<Vec<PathCommand>> {
    rects
        .iter()
        .map(|&(xs, ys, xe, ye)| {
            let fx0 = fx(xs as f64);
            let fx1 = fx(xe as f64);
            // Y は画像座標(Y下向き)をフォント座標(Y上向き)に反転する
            let fy_top = fy(ys as f64);
            let fy_bot = fy(ye as f64);

            vec![
                PathCommand::MoveTo { x: fx0, y: fy_top },
                PathCommand::LineTo { x: fx1, y: fy_top },
                PathCommand::LineTo { x: fx1, y: fy_bot },
                PathCommand::LineTo { x: fx0, y: fy_bot },
                PathCommand::Close { x: fx0, y: fy_top },
            ]
        })
        .collect()
}

/// 二値化セルから矩形群を抽出する（アップスケール → ランレングス → 縦マージ → ハングガード）。
///
/// 戻り値: (矩形リスト(x_start, y_start, x_end, y_end)（アップスケール後座標）, アップスケール後の幅, 高さ)。
/// 黒が無い・入力不正・矩形爆発（MAX_RECTS 超）の場合は None。
fn extract_rects(binary_sauvola: &[u8], w: u32, h: u32) -> Option<(Vec<(u32, u32, u32, u32)>, u32, u32)> {
    if w == 0 || h == 0 || binary_sauvola.len() < (w as usize) * (h as usize) {
        return None;
    }

    // 内部バイナリ: 1=黒(前景), 0=白(背景)
    let binary: Vec<u8> = binary_sauvola
        .iter()
        .map(|&v| if v == 0 { 1u8 } else { 0u8 })
        .collect();

    // 2: 2倍アップスケール（nearest neighbor）— ドットを細かくしてギザギザを目立ちにくくする
    const UPSCALE: u32 = 2;
    let uw = w * UPSCALE;
    let uh = h * UPSCALE;
    let upscaled: Vec<u8> = {
        let mut buf = vec![0u8; (uw * uh) as usize];
        for y in 0..uh {
            for x in 0..uw {
                buf[(y * uw + x) as usize] = binary[((y / UPSCALE) * w + x / UPSCALE) as usize];
            }
        }
        buf
    };

    // 5: ランレングス抽出（アップスケール後の座標で）
    let mut runs: Vec<(u32, u32, u32)> = Vec::new(); // (y, x_start, x_end)
    for y in 0..uh {
        let mut x = 0u32;
        while x < uw {
            if upscaled[(y * uw + x) as usize] != 1 {
                x += 1;
                continue;
            }
            let run_start = x;
            while x < uw && upscaled[(y * uw + x) as usize] == 1 {
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

    if rects.is_empty() {
        return None;
    }

    // 6.5: ハングガード。シアン/影残骸が二値化をすり抜け、行ごとに幅が ±MERGE_TOLERANCE を
    // 超えて揺れると縦マージが効かず矩形数が爆発し、opentype.js のグリフ書き出しが実質ハングする
    // （#82 のハング再発経路）。正常グリフは縦マージ後せいぜい数百矩形（典型 195-455 cmd/glyph）。
    // 上限を大きく超えたものは「ノイズ過多で破綻」とみなし、グリフを空に倒して安全側に倒す。
    const MAX_RECTS: usize = 4000;
    if rects.len() > MAX_RECTS {
        return None;
    }

    Some((rects, uw, uh))
}

/// セル画像を二値化し、品質ゲート（#110）を通した結果を返す。
///
/// 戻り値: (Sauvola 形式のバイナリ: 0=黒/255=白, 品質情報)
///
/// 処理順: グレー化 → CLAHE → Sauvola → モルフォロジ open-close →
/// **品質ゲート（境界接触成分の除去 + 面積フィルタ）** → インクブリード補正。
/// ゲートをインクブリード補正（1px erosion）より前に置くのは、erosion で
/// 残渣の境界接触が1px後退して検出帯から外れるのを防ぐため。
pub fn binarize_with_quality(img: &RgbaImage) -> (Vec<u8>, CellQuality) {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return (Vec::new(), CellQuality::empty());
    }
    let gray = rgba_to_gray_pub(img);
    let gray = apply_clahe_pub(&gray, w, h);
    let binary = sauvola_binarize_pub(&gray, w, h, SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB);
    let mut binary = morphological_open_close(&binary, w, h);
    let quality = apply_cell_quality_gate(&mut binary, w, h);
    let binary = compensate_ink_bleed(&binary, w, h);
    (binary, quality)
}

/// Sauvola 形式バイナリ（0=黒/255=白）を白背景+黒ストロークの RGBA に変換する
pub fn binary_to_rgba(binary: &[u8], w: u32, h: u32) -> RgbaImage {
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

/// セル画像を二値化済み白背景+黒ストロークの RGBA として返す（プレビュー・ベクター化入力用）
pub fn binarize_to_rgba(img: &RgbaImage) -> RgbaImage {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return img.clone();
    }
    let (binary, _quality) = binarize_with_quality(img);
    binary_to_rgba(&binary, w, h)
}

// ── SVG 出力（CLI の検証用） ──

/// パス配列をシンプルな SVG 文字列に変換する（デバッグ可視化用）
///
/// viewBox はセル crop 全域が写る em 範囲（#111 固定変換後は x∈[-100,1100],
/// y∈[-220,980]）。descender 領域（y<0）に置かれたインクも切れずに表示される。
#[cfg(not(target_arch = "wasm32"))]
pub fn paths_to_svg(paths: &[Vec<PathCommand>]) -> String {
    let (x_min, y_min, x_max, y_max) = crop_em_bounds();
    let vb_w = (x_max - x_min) as i32;
    let vb_h = (y_max - y_min) as i32;
    // フォント座標(Y上向き) → SVG 座標(Y下向き): y_svg = y_max - y_font
    let flip = |y: f64| y_max - y;
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x_min:.0} 0 {vb_w} {vb_h}\" width=\"400\" height=\"400\">\n"
    ));
    out.push_str(&format!(
        "  <rect x=\"{x_min:.0}\" width=\"100%\" height=\"100%\" fill=\"white\"/>\n"
    ));

    // 全サブパスを1つの <path> にまとめて fill で塗りつぶす
    let mut d = String::new();
    for path in paths {
        for cmd in path {
            match cmd {
                PathCommand::MoveTo { x, y } => {
                    d.push_str(&format!("M{x:.0} {y:.0} ", y = flip(*y)));
                }
                PathCommand::LineTo { x, y } => {
                    d.push_str(&format!("L{x:.0} {y:.0} ", y = flip(*y)));
                }
                // ランレングス方式では CurveTo は生成されないが、インポートフォントのパス表示用に残す
                PathCommand::CurveTo { x, y, cp1x, cp1y, cp2x, cp2y } => {
                    d.push_str(&format!(
                        "C{cp1x:.0} {cp1y:.0} {cp2x:.0} {cp2y:.0} {x:.0} {y:.0} ",
                        cp1y = flip(*cp1y),
                        cp2y = flip(*cp2y),
                        y = flip(*y),
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

    /// Sauvola 形式（0=黒/255=白）の合成バイナリを作る。rects は (x0, y0, x1, y1) 半開区間
    fn make_binary(w: u32, h: u32, rects: &[(u32, u32, u32, u32)]) -> Vec<u8> {
        let mut buf = vec![255u8; (w * h) as usize];
        for &(x0, y0, x1, y1) in rects {
            for y in y0..y1 {
                for x in x0..x1 {
                    buf[(y * w + x) as usize] = 0;
                }
            }
        }
        buf
    }

    /// 全パスの bbox (min_x, min_y, max_x, max_y) を集計
    fn paths_bbox(paths: &[Vec<PathCommand>]) -> (f64, f64, f64, f64) {
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for path in paths {
            for cmd in path {
                if let PathCommand::MoveTo { x, y } | PathCommand::LineTo { x, y } = cmd {
                    min_x = min_x.min(*x);
                    max_x = max_x.max(*x);
                    min_y = min_y.min(*y);
                    max_y = max_y.max(*y);
                }
            }
        }
        (min_x, min_y, max_x, max_y)
    }

    // ── セル→em 固定変換（#111） ──
    //
    // テストは 120×120px の合成 crop を使う。crop は 12mm 四方（CELL_CROP_SIZE）なので
    // ちょうど 10px/mm になり、期待値が整数で書ける:
    //   em_x = (px/10 - 1.0mm) * 100,  em_y = -120 + (11.0mm - py/10) * 100

    #[test]
    fn fixed_transform_maps_punctuation_position() {
        // 内枠左下（句読点相当）の小矩形: px x∈[15,35), y∈[85,105)
        // = crop 内 mm で x∈[1.5,3.5], y∈[8.5,10.5]（内枠は x,y∈[1.0,11.0]）
        // 期待 em: x∈[50,250], y∈[-70,130] — 左下・小さいまま写り、中央に拡大されない
        let binary = make_binary(120, 120, &[(15, 85, 35, 105)]);
        let paths = vectorize_binary(&binary, 120, 120);
        assert!(!paths.is_empty());
        let (min_x, min_y, max_x, max_y) = paths_bbox(&paths);
        assert_eq!((min_x, max_x), (50.0, 250.0), "x が書いた位置のまま写るべき");
        assert_eq!((min_y, max_y), (-70.0, 130.0), "y がベースライン付近の低い位置に写るべき");
    }

    #[test]
    fn fixed_transform_uses_descender_region() {
        // 内枠下端まで届く成分（descender 相当）: px y∈[90,110) = mm y∈[9.0,11.0]
        // 下端は内枠下端 = EMBOX_BOTTOM_Y(-120) に写る。y<0 の descender 領域が実際に使われる
        let binary = make_binary(120, 120, &[(20, 90, 40, 110)]);
        let paths = vectorize_binary(&binary, 120, 120);
        let (min_x, min_y, max_x, max_y) = paths_bbox(&paths);
        assert_eq!((min_x, max_x), (100.0, 300.0));
        assert_eq!(max_y, 80.0);
        assert_eq!(min_y, layout::EMBOX_BOTTOM_Y, "内枠下端は EMBOX_BOTTOM_Y に写るべき");
        assert!(min_y < 0.0, "descender 領域（y<0）が使われるべき");
    }

    #[test]
    fn fixed_transform_preserves_scale() {
        // 1mm 角と 4mm 角は em で 100 / 400 units — bbox 正規化のような等倍化が起きない
        let small = make_binary(120, 120, &[(30, 30, 40, 40)]);
        let large = make_binary(120, 120, &[(30, 30, 70, 70)]);
        let (s_min_x, _, s_max_x, _) = paths_bbox(&vectorize_binary(&small, 120, 120));
        let (l_min_x, _, l_max_x, _) = paths_bbox(&vectorize_binary(&large, 120, 120));
        assert_eq!(s_max_x - s_min_x, 100.0, "1mm 角 → 100 units");
        assert_eq!(l_max_x - l_min_x, 400.0, "4mm 角 → 400 units");
        assert_eq!(s_min_x, l_min_x, "同じ書き出し位置は同じ em 位置に写る");
    }

    #[test]
    fn baseline_guide_maps_to_em_zero() {
        // 定数の整合: ベースラインガイド（内枠下端の 1.2mm 上）は em の y=0 に写る
        assert!(
            (layout::GUIDE_BASELINE_OFFSET_MM * em_units_per_mm() + layout::EMBOX_BOTTOM_Y).abs()
                < 1e-9,
            "GUIDE_BASELINE_OFFSET_MM と EMBOX_BOTTOM_Y の関係式が崩れている"
        );
        // 実座標でも確認: 下端がベースライン高（crop 内 mm 9.8 = px 98）に接する矩形
        let binary = make_binary(120, 120, &[(50, 78, 70, 98)]);
        let (_, min_y, _, _) = paths_bbox(&vectorize_binary(&binary, 120, 120));
        assert_eq!(min_y, 0.0, "ベースラインに接する成分の下端は y=0 に写るべき");
    }

    #[test]
    fn crop_em_bounds_matches_layout() {
        // crop 全域の em 範囲（SVG viewBox 用）。既定レイアウトで (-100, -220, 1100, 980)
        let (x_min, y_min, x_max, y_max) = crop_em_bounds();
        assert_eq!((x_min, y_min, x_max, y_max), (-100.0, -220.0, 1100.0, 980.0));
    }

    #[test]
    fn bbox_fit_rescue_centers_offset_rect() {
        // opt-in 救済（旧 #53 方式）の判断ロジック保存テスト:
        // 右下に寄った矩形が em-square 中央に EM_FIT_SIZE で正規化される
        let binary = make_binary(100, 100, &[(80, 80, 95, 95)]);
        let paths = vectorize_binary_bbox_fit(&binary, 100, 100);
        assert!(!paths.is_empty());
        let (min_x, min_y, max_x, max_y) = paths_bbox(&paths);

        // 左右マージンが均等（±2 units 以内）で EM_FIT_SIZE 相当にスケールされている
        let left_margin = min_x;
        let right_margin = UNITS_PER_EM - max_x;
        assert!(
            (left_margin - right_margin).abs() < 3.0,
            "左右マージン差: L={left_margin}, R={right_margin}"
        );
        let width = max_x - min_x;
        assert!(
            (width - EM_FIT_SIZE).abs() < 3.0,
            "幅がEM_FIT_SIZE付近になるはず: 実際={width}"
        );

        // 上下マージンも均等
        let top_margin = GLYPH_HEIGHT - max_y;
        let bottom_margin = min_y;
        assert!(
            (top_margin - bottom_margin).abs() < 3.0,
            "上下マージン差: T={top_margin}, B={bottom_margin}"
        );
    }

    #[test]
    fn dense_unmergeable_noise_returns_empty() {
        // 縦マージが効かない高密度ノイズ（多数の孤立した小ブロック）を作り、
        // 矩形数が MAX_RECTS を超えたときにグリフが空へ倒れる（=ハングしない）ことを確認する。
        // 3x3 の黒ブロックを 6px グリッドに敷き詰めると各ブロックが独立矩形になり爆発する。
        let mut img = make_image(420, 420, Rgba([255, 255, 255, 255]));
        for gy in 0..70 {
            for gx in 0..70 {
                let bx = gx * 6;
                let by = gy * 6;
                for dy in 0..3 {
                    for dx in 0..3 {
                        img.put_pixel(bx + dx, by + dy, Rgba([0, 0, 0, 255]));
                    }
                }
            }
        }
        let paths = vectorize_glyph(&img);
        assert!(
            paths.is_empty(),
            "矩形爆発時はハングガードで空に倒すべき: 実際={}",
            paths.len()
        );
    }

    #[test]
    fn vectorize_strips_border_residue_keeps_stroke_intact() {
        // #110: セル境界に接触する細い残渣（枠・罫線の消し残り代理）が
        // ベクター化結果に混入せず、ストロークのパスは残渣なしの画像と一致する。
        let mut clean = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 30..70 {
            for x in 30..70 {
                clean.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let mut residue = clean.clone();
        // 上辺に接触する 3px 厚の横線（罫線残渣風）
        for y in 0..3 {
            for x in 10..90 {
                residue.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        // 左辺に接触する 3px 厚の縦線（シアン枠残渣風）
        for y in 20..80 {
            for x in 0..3 {
                residue.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        let clean_paths = vectorize_glyph(&clean);
        let residue_paths = vectorize_glyph(&residue);
        assert!(!clean_paths.is_empty());

        let clean_json = serde_json::to_string(&clean_paths).unwrap();
        let residue_json = serde_json::to_string(&residue_paths).unwrap();
        assert_eq!(
            residue_json, clean_json,
            "残渣入りセルのベクター化結果は残渣なしと一致するべき（残渣が混入しない）"
        );
    }

    #[test]
    fn binarize_with_quality_flags_border_residue() {
        // 残渣入りセルは needs_review が立ち、除去後のバイナリに境界帯の黒が残らない
        let mut img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 40..60 {
            for x in 40..60 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        for y in 0..3 {
            for x in 10..90 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        let (binary, quality) = binarize_with_quality(&img);
        assert!(quality.needs_review, "境界接触残渣を除去したら要確認が立つべき");
        assert!(quality.removed_components >= 1);
        assert_eq!(quality.kept_components, 1, "ストローク成分だけが残るべき");
        // 上端3行に黒が残っていない
        for y in 0..3u32 {
            for x in 0..100u32 {
                assert_eq!(binary[(y * 100 + x) as usize], 255, "({x},{y}) に残渣が残留");
            }
        }

        // 対照: 残渣なしのクリーンセルでは要確認は立たない
        let mut clean = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 40..60 {
            for x in 40..60 {
                clean.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let (_binary, clean_quality) = binarize_with_quality(&clean);
        assert!(!clean_quality.needs_review, "クリーンセルは要確認なし");
    }

    #[test]
    fn vectorize_binary_short_buffer_returns_empty() {
        // binary.len() < w*h の不正入力は空 Vec（パニックしない）
        let short = vec![0u8; 5];
        let paths = vectorize_binary(&short, 4, 4);
        assert!(paths.is_empty(), "不正長入力は空パスを返すべき");
    }

    #[test]
    fn gate_passes_residue_floating_off_border() {
        // 既知の限界の固定化: 境界から4px浮いた残渣線は帯（2px）に接触しないため
        // ゲートを素通りし、needs_review も立たない。TPS 補正残差（0.5〜1mm ≈ 6〜12px）で
        // 残渣が帯外へずれるケースの検出は #113 のスコープ（本テストは現状仕様の回帰検知用）。
        let mut img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        // 内側ストローク
        for y in 40..60 {
            for x in 40..60 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        // 境界から4px浮いた3px厚の横線（面積 ≥ MIN_SPECK_AREA なので面積フィルタにも掛からない）
        for y in 4..7 {
            for x in 10..90 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }

        let (binary, quality) = binarize_with_quality(&img);
        assert!(!quality.needs_review, "帯外の浮き残渣では要確認は立たない（現状仕様）");
        assert_eq!(quality.removed_components, 0);
        assert_eq!(quality.kept_components, 2, "ストロークと浮き残渣の両方が残る");
        // 残渣線が残留している（rows 3..8 に黒がある）
        let residue_black = (3u32..8)
            .flat_map(|y| (0u32..100).map(move |x| (y, x)))
            .filter(|&(y, x)| binary[(y * 100 + x) as usize] == 0)
            .count();
        assert!(residue_black > 0, "帯外の浮き残渣はゲート素通りで残留する（現状仕様）");
    }

    #[test]
    fn vertical_merge_reduces_rect_count() {
        // 白背景に縦10px×横40pxの縦棒を描画
        // 縦マージにより40行のランが少数の矩形に結合されるはず
        let mut img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 30..70 {
            for x in 45..55 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let paths = vectorize_glyph(&img);
        // 2x アップスケール後は80行のランが出るが、同一幅なので縦マージで大幅に減るはず
        // マージなしなら80パス、マージありなら数パスに収まる
        assert!(
            paths.len() < 20,
            "縦マージにより矩形数は大幅に減るはず: 実際={}",
            paths.len()
        );
        assert!(
            !paths.is_empty(),
            "黒ピクセルがあるのでパスは0にならない"
        );
    }
}
