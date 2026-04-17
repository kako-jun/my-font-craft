/// セル画像（RGBA）からグリフのベジェパスを抽出するモジュール
///
/// TypeScript 側の src/lib/vectorizer/contour.ts の完全移植。
/// 二値化は Sauvola（cell.rs 共通）を使う。画像処理は Rust 側で完結させ、
/// JS は得られた PathCommand 配列を opentype.js の Path に流し込むだけにする。
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

#[derive(Debug, Clone, Copy)]
struct Pt {
    x: f64,
    y: f64,
}

// ── エントリポイント ──

/// セル画像（RGBA）→ パス配列
///
/// 1. グレー化
/// 2. CLAHE
/// 3. Sauvola 二値化（0=黒, 255=白）
/// 4. モルフォロジ open-close
/// 5. 輪郭抽出
/// 6. Douglas-Peucker 簡略化
/// 7. 画像座標 → フォント座標へ正規化
/// 8. ベジェ曲線に変換
/// グリフあたりの最大コマンド数。TTF のサイズ膨張・opentype.js の toArrayBuffer ハング防止
/// 通常のフォントは 50〜200 cmd/glyph。これを超えたら DP epsilon を段階的に上げ、
/// それでも超える場合は小さいサブパスを切り捨てる
const MAX_COMMANDS_PER_GLYPH: usize = 200;

pub fn vectorize_glyph(img: &RgbaImage) -> Vec<Vec<PathCommand>> {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return Vec::new();
    }

    // 1-4: 二値化（内部バイナリ: 1=黒(前景), 0=白(背景)）
    let binary = binarize_for_contour(img);

    // 5: 輪郭抽出
    let contours = extract_contours(&binary, w as i32, h as i32);

    // 6: Douglas-Peucker（総コマンド数が上限を超える場合は epsilon を段階的に上げて再簡略化）
    let min_wh = (w.min(h)) as f64;
    let base_epsilon = (min_wh / 80.0).max(1.0);
    let mut simplified: Vec<Vec<Pt>> = contours
        .iter()
        .map(|c| douglas_peucker(c, base_epsilon))
        .collect();

    let mut epsilon = base_epsilon;
    for _ in 0..4 {
        let total: usize = simplified.iter().map(|c| c.len()).sum();
        if total <= MAX_COMMANDS_PER_GLYPH {
            break;
        }
        epsilon *= 1.5;
        simplified = contours
            .iter()
            .map(|c| douglas_peucker(c, epsilon))
            .collect();
    }

    // それでも超えるなら小さいサブパスから切り捨てる
    let total_after: usize = simplified.iter().map(|c| c.len()).sum();
    if total_after > MAX_COMMANDS_PER_GLYPH {
        simplified.sort_by(|a, b| b.len().cmp(&a.len()));
        let mut running = 0usize;
        simplified.retain(|c| {
            if running + c.len() <= MAX_COMMANDS_PER_GLYPH {
                running += c.len();
                true
            } else {
                false
            }
        });
    }

    // 7: 正規化
    let normalized: Vec<Vec<Pt>> = simplified
        .into_iter()
        .map(|c| normalize_contour(&c, w as f64, h as f64))
        .collect();

    // 8: ベジェ変換
    normalized.into_iter().map(|c| contour_to_path(&c)).collect()
}

/// セル画像を二値化してフラグ配列に変換する（1=黒=前景, 0=白=背景）
fn binarize_for_contour(img: &RgbaImage) -> Vec<u8> {
    let w = img.width();
    let h = img.height();
    let gray = rgba_to_gray_pub(img);
    let gray = apply_clahe_pub(&gray, w, h);
    let binary = sauvola_binarize_pub(&gray, w, h, SAUVOLA_K_PUB, SAUVOLA_WINDOW_PUB);
    let binary = morphological_open_close(&binary, w, h);
    // Sauvola 出力は 0=黒, 255=白。内部では TS 版と同じく 1=前景(黒) のフラグに変換
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

// ── 輪郭抽出（境界追跡） ──

fn extract_contours(binary: &[u8], w: i32, h: i32) -> Vec<Vec<Pt>> {
    let w_usize = w as usize;
    let h_usize = h as usize;
    let mut visited = vec![0u8; w_usize * h_usize];
    let mut contours: Vec<Vec<Pt>> = Vec::new();

    // 8方向
    let dx = [1i32, 1, 0, -1, -1, -1, 0, 1];
    let dy = [0i32, 1, 1, 1, 0, -1, -1, -1];

    let is_fg = |binary: &[u8], x: i32, y: i32| -> bool {
        if x < 0 || y < 0 || x >= w || y >= h {
            return false;
        }
        binary[(y as usize) * w_usize + x as usize] == 1
    };

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let idx = (y as usize) * w_usize + x as usize;
            if binary[idx] != 1 || visited[idx] != 0 {
                continue;
            }

            // 境界ピクセルかチェック
            let mut is_border = false;
            for d in 0..8 {
                let nx = x + dx[d];
                let ny = y + dy[d];
                if !is_fg(binary, nx, ny) {
                    is_border = true;
                    break;
                }
            }
            if !is_border {
                continue;
            }

            // 境界追跡
            let mut contour: Vec<Pt> = Vec::new();
            let mut cx = x;
            let mut cy = y;
            let mut dir: i32 = 0;
            let start_x = x;
            let start_y = y;
            let mut steps: i64 = 0;
            let max_steps: i64 = (w as i64) * (h as i64);

            loop {
                contour.push(Pt {
                    x: cx as f64,
                    y: cy as f64,
                });
                visited[(cy as usize) * w_usize + cx as usize] = 1;

                let mut found = false;
                for i in 0..8 {
                    let nd = (((dir + 6 + i) % 8) + 8) % 8; // 左回り
                    let nd_usize = nd as usize;
                    let nx = cx + dx[nd_usize];
                    let ny = cy + dy[nd_usize];
                    if nx >= 0 && nx < w && ny >= 0 && ny < h && is_fg(binary, nx, ny) {
                        // 境界ピクセル確認
                        let mut nb = false;
                        for d2 in 0..8 {
                            let nnx = nx + dx[d2];
                            let nny = ny + dy[d2];
                            if !is_fg(binary, nnx, nny) {
                                nb = true;
                                break;
                            }
                        }
                        if nb {
                            cx = nx;
                            cy = ny;
                            dir = nd;
                            found = true;
                            break;
                        }
                    }
                }

                if !found {
                    break;
                }
                steps += 1;
                if (cx == start_x && cy == start_y) || steps >= max_steps {
                    break;
                }
            }

            // 連結成分全体を visited にマーク
            // （境界追跡だけだと太いストロークの反対側エッジから再トレースされる）
            let mut stack = vec![(start_x, start_y)];
            while let Some((sx, sy)) = stack.pop() {
                if sx < 0 || sy < 0 || sx >= w || sy >= h {
                    continue;
                }
                let sidx = (sy as usize) * w_usize + sx as usize;
                if binary[sidx] != 1 || visited[sidx] != 0 {
                    continue;
                }
                visited[sidx] = 1;
                for d in 0..8 {
                    stack.push((sx + dx[d], sy + dy[d]));
                }
            }

            if contour.len() >= 10 {
                contours.push(contour);
            }
        }
    }

    // bbox-ベース重複除去: 同じ境界を複数回なぞった輪郭を1本にまとめる
    dedup_contours(contours)
}

/// 同一 bbox・類似サイズの輪郭を重複として除去
fn dedup_contours(contours: Vec<Vec<Pt>>) -> Vec<Vec<Pt>> {
    // bbox は1度だけ計算する
    let with_bbox: Vec<(Bbox, Vec<Pt>)> = contours
        .into_iter()
        .map(|c| (compute_bbox(&c), c))
        .collect();

    let mut kept: Vec<(Bbox, Vec<Pt>)> = Vec::new();
    for (bbox, c) in with_bbox {
        let is_dup = kept.iter().any(|(existing_bbox, existing)| {
            let bbox_same = (bbox.min_x - existing_bbox.min_x).abs() < 2.0
                && (bbox.min_y - existing_bbox.min_y).abs() < 2.0
                && (bbox.max_x - existing_bbox.max_x).abs() < 2.0
                && (bbox.max_y - existing_bbox.max_y).abs() < 2.0;
            if !bbox_same {
                return false;
            }
            let ratio = c.len() as f64 / existing.len() as f64;
            ratio > 0.5 && ratio < 2.0
        });
        if !is_dup {
            kept.push((bbox, c));
        }
    }
    kept.into_iter().map(|(_, c)| c).collect()
}

#[derive(Debug, Clone, Copy)]
struct Bbox {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

fn compute_bbox(c: &[Pt]) -> Bbox {
    let mut b = Bbox {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };
    for p in c {
        if p.x < b.min_x { b.min_x = p.x; }
        if p.y < b.min_y { b.min_y = p.y; }
        if p.x > b.max_x { b.max_x = p.x; }
        if p.y > b.max_y { b.max_y = p.y; }
    }
    b
}

// ── Douglas-Peucker（反復版） ──

fn douglas_peucker(points: &[Pt], epsilon: f64) -> Vec<Pt> {
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }

    let mut keep = vec![0u8; n];
    keep[0] = 1;
    keep[n - 1] = 1;

    let mut stack: Vec<(usize, usize)> = vec![(0, n - 1)];

    while let Some((start, end)) = stack.pop() {
        let mut max_dist = 0.0f64;
        let mut max_idx = start;

        for i in (start + 1)..end {
            let d = perpendicular_dist(points[i], points[start], points[end]);
            if d > max_dist {
                max_dist = d;
                max_idx = i;
            }
        }

        if max_dist > epsilon {
            keep[max_idx] = 1;
            if max_idx - start > 1 {
                stack.push((start, max_idx));
            }
            if end - max_idx > 1 {
                stack.push((max_idx, end));
            }
        }
    }

    points
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i] == 1)
        .map(|(_, p)| *p)
        .collect()
}

fn perpendicular_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len == 0.0 {
        return ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
    }
    (dy * p.x - dx * p.y + b.x * a.y - b.y * a.x).abs() / len
}

// ── 画像座標 → フォント座標 ──

fn normalize_contour(points: &[Pt], img_w: f64, img_h: f64) -> Vec<Pt> {
    let scale = GLYPH_HEIGHT / img_h;
    let offset_x = (UNITS_PER_EM - img_w * scale) / 2.0;
    points
        .iter()
        .map(|p| Pt {
            x: (p.x * scale + offset_x).round(),
            y: (GLYPH_HEIGHT - p.y * scale).round(),
        })
        .collect()
}

// ── 輪郭 → ベジェパス ──

fn contour_to_path(points: &[Pt]) -> Vec<PathCommand> {
    if points.len() < 2 {
        return Vec::new();
    }
    let mut commands: Vec<PathCommand> = Vec::new();
    commands.push(PathCommand::MoveTo {
        x: points[0].x,
        y: points[0].y,
    });

    let n = points.len();
    let mut i = 1usize;
    while i < n {
        if i + 2 < n {
            let p0 = points[i - 1];
            let p1 = points[i];
            let p2 = points[i + 1];
            let p3 = points[i + 2];
            commands.push(PathCommand::CurveTo {
                x: p3.x,
                y: p3.y,
                cp1x: p0.x + (p1.x - p0.x) * 0.66,
                cp1y: p0.y + (p1.y - p0.y) * 0.66,
                cp2x: p3.x + (p2.x - p3.x) * 0.66,
                cp2y: p3.y + (p2.y - p3.y) * 0.66,
            });
            i += 3;
        } else {
            commands.push(PathCommand::LineTo {
                x: points[i].x,
                y: points[i].y,
            });
            i += 1;
        }
    }

    commands.push(PathCommand::Close {
        x: points[0].x,
        y: points[0].y,
    });
    commands
}

// ── SVG 出力（CLI の検証用） ──

/// パス配列をシンプルな SVG 文字列に変換する（デバッグ可視化用）
#[cfg(not(target_arch = "wasm32"))]
pub fn paths_to_svg(paths: &[Vec<PathCommand>]) -> String {
    // フォント座標系（Y軸が上向き、0-1000）を SVG 座標系（Y軸が下向き）に変換して描画
    let vb_size = UNITS_PER_EM as i32;
    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {vb_size} {vb_size}\" width=\"400\" height=\"400\">\n"
    ));
    out.push_str("  <rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n");
    // Y軸反転
    out.push_str(&format!("  <g transform=\"translate(0,{vb_size}) scale(1,-1)\">\n"));

    for path in paths {
        let mut d = String::new();
        for cmd in path {
            match cmd {
                PathCommand::MoveTo { x, y } => d.push_str(&format!("M{x:.1} {y:.1} ")),
                PathCommand::LineTo { x, y } => d.push_str(&format!("L{x:.1} {y:.1} ")),
                PathCommand::CurveTo {
                    x,
                    y,
                    cp1x,
                    cp1y,
                    cp2x,
                    cp2y,
                } => d.push_str(&format!(
                    "C{cp1x:.1} {cp1y:.1} {cp2x:.1} {cp2y:.1} {x:.1} {y:.1} "
                )),
                PathCommand::Close { .. } => d.push_str("Z "),
            }
        }
        out.push_str(&format!(
            "    <path d=\"{d}\" stroke=\"black\" stroke-width=\"2\" fill=\"none\"/>\n"
        ));
    }

    out.push_str("  </g>\n</svg>\n");
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
        // Sauvola は均一領域を背景扱いするので、黒ピクセルなし → 輪郭なし
        let img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        let paths = vectorize_glyph(&img);
        assert!(paths.is_empty(), "均一白は空のパスを返すべき");
    }

    #[test]
    fn uniform_black_does_not_panic() {
        // Sauvola は均一領域ではコントラストなしで背景扱いする
        // 結果はノイズ的だがパニックしないことが重要
        let img = make_image(100, 100, Rgba([0, 0, 0, 255]));
        let _paths = vectorize_glyph(&img);
    }

    #[test]
    fn black_rect_on_white_produces_closed_path() {
        // 白背景に中央の黒矩形
        let mut img = make_image(100, 100, Rgba([255, 255, 255, 255]));
        for y in 30..70 {
            for x in 30..70 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let paths = vectorize_glyph(&img);
        assert!(!paths.is_empty(), "黒矩形に対して少なくとも1つのパスが返るべき");
        for path in &paths {
            // 必ず MoveTo で始まる
            assert!(
                matches!(path.first(), Some(PathCommand::MoveTo { .. })),
                "パスは MoveTo で始まるべき"
            );
            // 必ず Close で終わる
            assert!(
                matches!(path.last(), Some(PathCommand::Close { .. })),
                "パスは Close で終わるべき"
            );
        }
    }

    #[test]
    fn normalize_y_axis_flip() {
        // 画像座標 y=0（上端） → フォント座標で大きな y 値（GLYPH_HEIGHT に近い）
        // 画像座標 y=h-1（下端） → フォント座標で小さな y 値（0 に近い）
        let w = 100.0f64;
        let h = 100.0f64;
        let pts = vec![
            Pt { x: 50.0, y: 0.0 },
            Pt {
                x: 50.0,
                y: h - 1.0,
            },
        ];
        let norm = normalize_contour(&pts, w, h);
        assert!(
            norm[0].y > norm[1].y,
            "Y軸反転: 画像の上（y=0）がフォント座標で大きい値になるべき: {} > {}",
            norm[0].y,
            norm[1].y
        );
        // 画像上端 y=0 → GLYPH_HEIGHT
        assert!(
            (norm[0].y - GLYPH_HEIGHT).abs() < 2.0,
            "画像上端は GLYPH_HEIGHT に近いはず: {}",
            norm[0].y
        );
    }

    #[test]
    fn douglas_peucker_keeps_endpoints() {
        let pts = vec![
            Pt { x: 0.0, y: 0.0 },
            Pt { x: 1.0, y: 0.1 },
            Pt { x: 2.0, y: -0.1 },
            Pt { x: 3.0, y: 0.05 },
            Pt { x: 10.0, y: 0.0 },
        ];
        let out = douglas_peucker(&pts, 1.0);
        // 端点は必ず含まれる
        assert_eq!(out.first().unwrap().x, 0.0);
        assert_eq!(out.last().unwrap().x, 10.0);
        // 中間点は簡略化で消えてよい
        assert!(out.len() <= pts.len());
    }
}
