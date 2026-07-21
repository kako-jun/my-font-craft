/// セル画像（RGBA）からグリフのパスを抽出するモジュール
///
/// 二値化はハイブリッド（グローバル閾値 OR Sauvola、cell.rs 共通、#136）を使う。画像処理は Rust 側で完結させ、
/// JS は得られた PathCommand 配列を opentype.js の Path に流し込むだけにする。
///
/// ランレングス方式: 二値化画像の各行で黒ピクセルの連続区間（ラン）を検出し、
/// 各ランを四角形パスに変換する。二値化画像と100%同じ見た目が保証される。
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::cell::{
    apply_cell_quality_gate, apply_clahe_pub, binarize_hybrid_pub, compensate_ink_bleed,
    morphological_open_close, rgba_to_gray_pub, CellQuality,
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

// ── 座標変換（#111 のセル→em 固定アフィンを両方式で共有） ──

/// セル crop の画像座標(px, Y下向き)→ em 座標(units, Y上向き)の固定アフィン変換（#111）。
///
/// 内枠（10mm、書く領域）を em-square [0,1000] × [EMBOX_BOTTOM_Y, EMBOX_BOTTOM_Y+1000]
/// に写す。ランレングス方式（アップスケール後グリッド）と輪郭方式（原寸グリッド）で
/// **同一の写像**を使い、#111 の配置仕様（句読点左下・小書きかな小・descender）を
/// 方式に依らず保つ。mm 基準なのでグリッド解像度（pw/ph）に依存せず結果は一致する。
struct EmTransform {
    px_per_mm_x: f64,
    px_per_mm_y: f64,
    em: f64,
    inner_left_mm: f64,
    inner_bottom_mm: f64,
}

impl EmTransform {
    fn new(pw: u32, ph: u32) -> Self {
        let inner_left_mm = inner_left_in_crop_mm();
        Self {
            // 幅と高さは同じ mm（正方 crop）だが mm→px の丸めで px 数が異なりうるため軸ごとに算出
            px_per_mm_x: pw as f64 / layout::CELL_CROP_SIZE,
            px_per_mm_y: ph as f64 / layout::CELL_CROP_SIZE,
            em: em_units_per_mm(),
            inner_left_mm,
            inner_bottom_mm: inner_left_mm + layout::INNER_SIZE,
        }
    }

    fn fx(&self, px: f64) -> f64 {
        ((px / self.px_per_mm_x - self.inner_left_mm) * self.em).round()
    }

    fn fy(&self, py: f64) -> f64 {
        (layout::EMBOX_BOTTOM_Y + (self.inner_bottom_mm - py / self.px_per_mm_y) * self.em).round()
    }
}

// ── 輪郭ベクター化のパラメータ（#112） ──

/// Douglas-Peucker の許容誤差（原寸ピクセル）。二値化の階段状ジャギーを直線・曲線に
/// 畳み込むためのしきい値。大きいほど cmd/glyph が減るが、小さすぎると階段が残り、
/// 大きすぎると字形が崩れる。300dpi で 1px ≈ 0.085mm ≈ 8.5 em units。
const CONTOUR_DP_EPSILON_PX: f64 = 1.5;

/// このターン角（度）を超える頂点は「角（コーナー）」とみなして直線接続で残す。
/// 以下ならカーブの一部とみなして 3次ベジェで丸める。90°（矩形の角）は必ず残る。
const CONTOUR_CORNER_THRESHOLD_DEG: f64 = 62.0;

/// コーナー丸めのカット比率（隣接辺長に対する割合）。0.5 未満なら隣接する丸めが
/// 重ならず、丸め後の曲線は元の単純多角形の局所コーナー三角形内に収まる
/// = 新たな自己交差を生まない（#84 の自己交差崩壊の再発防止）。
const CONTOUR_CUT_FRACTION: f64 = 0.42;

/// 輪郭本数の暴走ガード。正常グリフは数〜数十本。これを超えたらノイズ過多とみなし
/// グリフを空へ倒す（ランレングスの MAX_RECTS と同じ安全側フォールバック）。
const MAX_CONTOURS: usize = 1024;

/// 単純化後の総頂点数の暴走ガード。opentype.js の書き出しが実質ハングするのを防ぐ。
const MAX_CONTOUR_POINTS: usize = 20000;

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

/// 二値化済みセル（Sauvola 形式: 0=黒/255=白、品質ゲート適用済み）→ パス配列（**輪郭方式**・既定）。
///
/// pipeline 側で二値化を1回だけ行い、プレビュー RGBA とベクター化の入力を
/// 完全に一致させるための分割エントリポイント。
///
/// 方式（#112）: **輪郭追跡 + 巻き方向管理**。ランレングス矩形（195〜455 cmd/glyph・
/// 階段状ジャギー・ノイズで矩形爆発）から脱却する。
/// 1. クラック追跡（前景/背景の境界を画素の辺に沿って追う）で閉ループ群を得る。
///    「前景を常に右に見る」規約により、外輪郭と穴は**必ず逆向き**に巻かれる
///    （Green の定理）。原寸グリッドで追うため矩形の角が整数座標に落ち、#111 の
///    固定変換をそのまま乗せると配置がランレングスと一致する。
/// 2. Douglas-Peucker で階段・直線を間引く（cmd/glyph を1桁削減）。
/// 3. 角を残しつつ緩い頂点を3次ベジェで丸める（自己交差を生まない局所丸め）。
/// 4. TrueType の nonzero winding 前提でパスを組む。外輪郭を CW・穴を CCW に揃える
///    ことで #84 の「evenodd 自己交差崩壊」を回避する。巻き方向はクラック追跡規約が
///    保証するため反転処理は不要（安全網としてテストで固定）。
///
/// 座標変換（#111）: **セル矩形→em の固定アフィン変換**（`EmTransform`）。
/// 入力画像は「セル外枠から CELL_CROP_MARGIN(1.5mm) 内側を crop した
/// CELL_CROP_SIZE(12mm) 四方」である前提（cell.rs extract_cell_image_raw）。
/// 内枠（10mm、書く領域）を em-square [0,1000] × [-120, 880] に写す（1mm = 100 units）。
///
/// ランレングス方式は `vectorize_binary_runlength`（フォールバック/デバッグ用）に温存。
pub fn vectorize_binary(binary_sauvola: &[u8], w: u32, h: u32) -> Vec<Vec<PathCommand>> {
    if w == 0 || h == 0 || binary_sauvola.len() < (w as usize) * (h as usize) {
        return Vec::new();
    }

    // 1: クラック追跡で境界閉ループ群（整数グリッド頂点、原寸座標）を得る
    let loops = trace_boundary_loops(binary_sauvola, w, h);
    if loops.is_empty() || loops.len() > MAX_CONTOURS {
        return Vec::new();
    }

    let t = EmTransform::new(w, h);
    let mut paths: Vec<Vec<PathCommand>> = Vec::with_capacity(loops.len());
    let mut total_pts = 0usize;

    for lp in &loops {
        // 2: Douglas-Peucker で単純化（原寸ピクセル座標のまま = 整数幾何で collinear を厳密除去）
        let simplified = douglas_peucker_closed(lp, CONTOUR_DP_EPSILON_PX);
        if simplified.len() < 3 {
            continue; // 面積ゼロ級の退化ループは捨てる
        }
        total_pts += simplified.len();
        if total_pts > MAX_CONTOUR_POINTS {
            return Vec::new(); // ハングガード
        }

        // 3: 頂点を em 座標へ（#111 の固定変換）。輪郭点にそのまま適用する
        let em_pts: Vec<(f64, f64)> = simplified
            .iter()
            .map(|&(x, y)| (t.fx(x as f64), t.fy(y as f64)))
            .collect();

        // 4: 角を残しつつ緩い頂点をベジェで丸めて閉パスを生成
        let path = smooth_contour_to_path(&em_pts);
        if path.len() >= 4 {
            // M + 2辺 + Z 以上のみ採用（退化を除外）
            paths.push(path);
        }
    }

    paths
}

/// 二値化バッファ（Sauvola 形式: 0=黒/255=白）の w×h 範囲に前景（黒）画素が
/// 1つでもあるかを返す。バッファ長が w×h に満たない不正入力は「インクなし」とみなす。
pub fn binary_has_ink(binary_sauvola: &[u8], w: u32, h: u32) -> bool {
    let n = (w as usize) * (h as usize);
    binary_sauvola.len() >= n && binary_sauvola[..n].iter().any(|&v| v == 0)
}

/// **採用セル**（judge_adoption が採用＝生セルにインクがあった）のベクター化を行い、
/// **パスが空**なら `quality.needs_review` を立てる（#112 / #108）。
///
/// この関数は採用セルに対してのみ呼ばれる。採用されたのにベクター化結果が空＝
/// グリフを生成できない以上、原因を問わず**必ず**要確認にする。これは次の両方を包含する:
///   - **MAX ガード発火** — MAX_CONTOURS / MAX_CONTOUR_POINTS / MAX_RECTS で空へ倒す
///     （このとき gated_binary にはインクが残っている）。
///   - **pre-gate 消失** — 生セルにインクはあったが、Sauvola 閾値処理や
///     morphological_open_close で品質ゲート到達**前**にストロークが消え、gated_binary が
///     既に空になっている（ゲートは 0 成分除去なので単独では needs_review を立てない）。
///
/// 判定は `paths.is_empty()` だけに基づき、`binary_has_ink(gated_binary)`（＝ゲート後に
/// インクが残ったか）には依存しない。ゲート後インクの有無で分岐すると pre-gate 消失
/// サブクラスが漏れて #108 の「黙って欠字」が再来するため（セルフレビュー指摘）。
///
/// pipeline（本番経路）と回帰テストで同一のこの関数を通すことで、サイレント欠字の
/// 検知ロジックが両者で必ず一致する。
pub fn vectorize_adopted_with_review(
    binary_sauvola: &[u8],
    w: u32,
    h: u32,
    quality: &mut CellQuality,
) -> Vec<Vec<PathCommand>> {
    let paths = vectorize_binary(binary_sauvola, w, h);
    if paths.is_empty() {
        quality.needs_review = true;
    }
    paths
}

/// 二値化済みセル → パス配列（**ランレングス方式**・フォールバック/デバッグ用）。
///
/// 2x アップスケール → 各行の黒連続区間を検出 → 縦マージ → 各矩形を M→L→L→L→Z の
/// 四角形パスに変換する。二値化画像と100%同じ見た目（階段状ジャギーも忠実再現）。
/// ノイズ混入時に矩形が爆発するため MAX_RECTS ハングガードを持つ。#112 で既定を
/// 輪郭方式（`vectorize_binary`）に切り替え、こちらは温存に格下げ。
pub fn vectorize_binary_runlength(binary_sauvola: &[u8], w: u32, h: u32) -> Vec<Vec<PathCommand>> {
    let Some((rects, uw, uh)) = extract_rects(binary_sauvola, w, h) else {
        return Vec::new();
    };
    // 画像座標(px, Y下向き) → em 座標(units, Y上向き)。輪郭方式と同一の固定変換を共有する
    let t = EmTransform::new(uw, uh);
    rects_to_paths(&rects, |px| t.fx(px), |py| t.fy(py))
}

// ── 輪郭追跡（クラック追跡 + 巻き方向規約） ──

/// 二値化セル（0=黒/255=白）の前景境界を、画素の辺（クラック）に沿って追い、
/// 閉ループ群（整数グリッド頂点列、原寸座標、末尾に始点重複なし）を返す。
///
/// 規約「前景を常に右に見る」で各前景セルの露出辺を時計回り（画像 Y 下向き）に張る。
/// これにより外輪郭と穴は自動的に逆向きに巻かれ、nonzero winding で正しく塗れる。
/// #111 の Y 反転変換を通すと外輪郭は font 空間で CW・穴は CCW になる（テストで固定）。
fn trace_boundary_loops(bin: &[u8], w: u32, h: u32) -> Vec<Vec<(i32, i32)>> {
    let wi = w as i32;
    let hi = h as i32;
    let fg = |x: i32, y: i32| -> bool {
        x >= 0 && y >= 0 && x < wi && y < hi && bin[(y as usize) * (w as usize) + x as usize] == 0
    };

    // 有向境界辺の隣接リスト: 始点 -> 終点群。決定論のため BTreeMap（キー昇順）。
    let mut adj: BTreeMap<(i32, i32), Vec<(i32, i32)>> = BTreeMap::new();
    for y in 0..hi {
        for x in 0..wi {
            if !fg(x, y) {
                continue;
            }
            // 各前景セル(x,y)は正方形 [x,x+1]×[y,y+1] を占める。背景と接する辺を
            // セル外周を時計回り（画像座標）に張る = 前景を右に見る。
            if !fg(x, y - 1) {
                adj.entry((x, y)).or_default().push((x + 1, y)); // 上辺: 左→右
            }
            if !fg(x + 1, y) {
                adj.entry((x + 1, y)).or_default().push((x + 1, y + 1)); // 右辺: 上→下
            }
            if !fg(x, y + 1) {
                adj.entry((x + 1, y + 1)).or_default().push((x, y + 1)); // 下辺: 右→左
            }
            if !fg(x - 1, y) {
                adj.entry((x, y + 1)).or_default().push((x, y)); // 左辺: 下→上
            }
        }
    }

    // ループ抽出: 辺を消費しながら閉路を追う
    let starts: Vec<(i32, i32)> = adj.keys().copied().collect();
    let mut loops: Vec<Vec<(i32, i32)>> = Vec::new();

    for &s in &starts {
        while adj.get(&s).is_some_and(|v| !v.is_empty()) {
            let mut pts: Vec<(i32, i32)> = Vec::new();
            let mut cur = s;
            let mut prev_dir: Option<(i32, i32)> = None;

            loop {
                pts.push(cur);
                let Some(outs) = adj.get_mut(&cur) else { break };
                if outs.is_empty() {
                    break;
                }
                // ピンチ頂点（対角前景が触れて出口が複数）は最も時計回りに曲がる辺を選ぶ。
                // 単純頂点は出口1つで一意。
                let ni = choose_next(outs, prev_dir, cur);
                let nxt = outs.remove(ni);
                prev_dir = Some((nxt.0 - cur.0, nxt.1 - cur.1));
                cur = nxt;
                if cur == s {
                    break; // 閉じた（始点は重複させない）
                }
            }

            if pts.len() >= 3 {
                loops.push(pts);
            }
        }
    }

    loops
}

/// ピンチ頂点での次辺選択: 入射方向 `din` に対し最も時計回り（画像 Y 下向き）に
/// 曲がる出口を選ぶ。前景を右に見る規約を保ち、ループ同士の交差を防ぐ。
fn choose_next(outs: &[(i32, i32)], din: Option<(i32, i32)>, cur: (i32, i32)) -> usize {
    if outs.len() == 1 || din.is_none() {
        return 0;
    }
    let (dix, diy) = din.unwrap();
    let mut best = 0usize;
    let mut best_key = f64::NEG_INFINITY;
    for (i, &(tx, ty)) in outs.iter().enumerate() {
        let (dox, doy) = (tx - cur.0, ty - cur.1);
        // 画像 Y 下向きでの時計回り度合い。cross>0 が時計回り側。
        let cross = (dix * doy - diy * dox) as f64;
        let dot = (dix * dox + diy * doy) as f64;
        // atan2 で [-π,π] の符号付き角にし、時計回り（cross>0）を大きく評価する
        let ang = cross.atan2(dot);
        if ang > best_key {
            best_key = ang;
            best = i;
        }
    }
    best
}

// ── Douglas-Peucker 単純化 ──

/// 閉多角形の Douglas-Peucker 単純化。極値点（最小 x, 次に最小 y = 真の角）を
/// アンカーに開多角形へ展開して DP し、始点の重複を落として閉多角形に戻す。
fn douglas_peucker_closed(pts: &[(i32, i32)], eps: f64) -> Vec<(i32, i32)> {
    let n = pts.len();
    if n < 4 {
        return pts.to_vec();
    }
    // アンカー = (x,y) 辞書順最小の頂点（必ず本物の角）
    let mut a = 0usize;
    for i in 1..n {
        if pts[i] < pts[a] {
            a = i;
        }
    }
    // アンカー始点で1周し、末尾にアンカーを重ねた開多角形にする
    let mut rot: Vec<(i32, i32)> = (0..n).map(|k| pts[(a + k) % n]).collect();
    rot.push(rot[0]);

    let kept = dp_open(&rot, eps);
    // kept は両端（同一点）を含む。末尾のアンカー重複を落として閉多角形へ
    let mut out = kept;
    out.pop();
    out
}

/// 開多角形の Douglas-Peucker。両端を保持し、区間内で線分から最も離れた点が
/// eps を超えれば分割再帰する。整数座標の垂線距離^2 で厳密比較する。
fn dp_open(pts: &[(i32, i32)], eps: f64) -> Vec<(i32, i32)> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let eps2 = eps * eps;
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;

    // 明示スタックで区間 [lo, hi] を処理（再帰の深さ暴走を避ける）
    let mut stack: Vec<(usize, usize)> = vec![(0, n - 1)];
    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo + 1 {
            continue;
        }
        let (ax, ay) = (pts[lo].0 as f64, pts[lo].1 as f64);
        let (bx, by) = (pts[hi].0 as f64, pts[hi].1 as f64);
        let dx = bx - ax;
        let dy = by - ay;
        let len2 = dx * dx + dy * dy;

        let mut far = lo;
        let mut far_d2 = -1.0f64;
        for (i, &(px_i, py_i)) in pts.iter().enumerate().take(hi).skip(lo + 1) {
            let px = px_i as f64;
            let py = py_i as f64;
            let d2 = if len2 <= f64::EPSILON {
                // 始終点が同一（閉じかけ）: 点距離^2
                let ex = px - ax;
                let ey = py - ay;
                ex * ex + ey * ey
            } else {
                // 線分への垂線距離^2（外積^2 / 長さ^2）
                let cross = dx * (py - ay) - dy * (px - ax);
                cross * cross / len2
            };
            if d2 > far_d2 {
                far_d2 = d2;
                far = i;
            }
        }

        if far_d2 > eps2 {
            keep[far] = true;
            stack.push((lo, far));
            stack.push((far, hi));
        }
    }

    (0..n).filter(|&i| keep[i]).map(|i| pts[i]).collect()
}

// ── 角保存ベジェ丸め ──

/// em 座標の閉多角形（末尾に始点重複なし）を、角を残しつつ緩い頂点を3次ベジェで
/// 丸めた閉パス（M / L / C / Z）に変換する。
///
/// 各頂点の入射辺と射出辺のなす角（ターン角）を測り、
/// CONTOUR_CORNER_THRESHOLD_DEG を超える鋭い頂点は角として直線接続で残す（矩形の
/// 90°は残る）。緩い頂点は隣接辺長の CONTOUR_CUT_FRACTION（<0.5）までカットした
/// 2点間を、制御点を頂点に置いた3次ベジェで結ぶ。カットが局所コーナー三角形に
/// 収まるため、単純多角形なら丸め後も自己交差しない（#84 の崩壊を回避）。
fn smooth_contour_to_path(v: &[(f64, f64)]) -> Vec<PathCommand> {
    let n = v.len();
    if n < 3 {
        // 退化: そのまま多角形として閉じる（呼び出し側で長さ判定して捨てる）
        let mut out: Vec<PathCommand> = Vec::new();
        if let Some(&(x, y)) = v.first() {
            out.push(PathCommand::MoveTo { x, y });
            for &(x, y) in &v[1..] {
                out.push(PathCommand::LineTo { x, y });
            }
            out.push(PathCommand::Close { x, y });
        }
        return out;
    }

    let threshold = CONTOUR_CORNER_THRESHOLD_DEG.to_radians();

    // 各頂点の in/out 点と分類を事前計算
    #[derive(Clone, Copy)]
    struct Node {
        sharp: bool,
        enter: (f64, f64),
        exit: (f64, f64),
        vertex: (f64, f64),
    }
    let mut nodes: Vec<Node> = Vec::with_capacity(n);
    for i in 0..n {
        let prev = v[(i + n - 1) % n];
        let cur = v[i];
        let next = v[(i + 1) % n];
        let din = (cur.0 - prev.0, cur.1 - prev.1);
        let dout = (next.0 - cur.0, next.1 - cur.1);
        let lin = (din.0 * din.0 + din.1 * din.1).sqrt();
        let lout = (dout.0 * dout.0 + dout.1 * dout.1).sqrt();

        if lin < f64::EPSILON || lout < f64::EPSILON {
            // 退化辺（重複点）は角として残す
            nodes.push(Node { sharp: true, enter: cur, exit: cur, vertex: cur });
            continue;
        }

        // ターン角: 入射方向と射出方向のなす角（0=直線、π=Uターン）
        let cross = din.0 * dout.1 - din.1 * dout.0;
        let dot = din.0 * dout.0 + din.1 * dout.1;
        let turn = cross.atan2(dot).abs();

        if turn > threshold {
            nodes.push(Node { sharp: true, enter: cur, exit: cur, vertex: cur });
        } else {
            // 隣接辺長の min にカット比率を掛けた長さだけ手前/先へ寄せる
            let c = CONTOUR_CUT_FRACTION * lin.min(lout);
            let enter = (cur.0 - c * din.0 / lin, cur.1 - c * din.1 / lin);
            let exit = (cur.0 + c * dout.0 / lout, cur.1 + c * dout.1 / lout);
            nodes.push(Node { sharp: false, enter, exit, vertex: cur });
        }
    }

    let r = |x: f64| x.round();
    let mut out: Vec<PathCommand> = Vec::with_capacity(n * 2 + 2);
    let start = nodes[0].enter;
    out.push(PathCommand::MoveTo { x: r(start.0), y: r(start.1) });

    for (i, nd) in nodes.iter().enumerate() {
        if nd.sharp {
            if i != 0 {
                out.push(PathCommand::LineTo { x: r(nd.vertex.0), y: r(nd.vertex.1) });
            }
        } else {
            if i != 0 {
                // 前ノードの exit から この頂点の enter まで直線
                out.push(PathCommand::LineTo { x: r(nd.enter.0), y: r(nd.enter.1) });
            }
            // enter -> exit を頂点を制御点にした3次ベジェで丸める
            out.push(PathCommand::CurveTo {
                x: r(nd.exit.0),
                y: r(nd.exit.1),
                cp1x: r(nd.vertex.0),
                cp1y: r(nd.vertex.1),
                cp2x: r(nd.vertex.0),
                cp2y: r(nd.vertex.1),
            });
        }
    }

    out.push(PathCommand::Close { x: r(start.0), y: r(start.1) });
    out
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
/// 戻り値: (0=黒/255=白のバイナリ, 品質情報)
///
/// 処理順: グレー化 → CLAHE → ハイブリッド二値化（グローバル閾値 OR Sauvola、#136）
/// → モルフォロジ open-close → **品質ゲート（境界接触成分の除去 + 面積フィルタ）**
/// → インクブリード補正。
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
    let binary = binarize_hybrid_pub(&gray, w, h);
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

// ── CLI 計測（#112） ──

/// 輪郭方式のコマンド数を3点計測する（CLI デバッグ用）:
/// `(単純化前, 単純化後=vectorize_binary出力, 現行ランレングス)`。
///
/// - 単純化前: クラック追跡の全頂点をそのまま多角形にした場合のコマンド数（M + (n-1)L + Z）
/// - 単純化後: Douglas-Peucker + ベジェ丸め適用後（本番出力）
/// - ランレングス: フォールバック方式の矩形コマンド数
#[cfg(not(target_arch = "wasm32"))]
pub fn vectorize_command_counts(binary_sauvola: &[u8], w: u32, h: u32) -> (usize, usize, usize) {
    let raw: usize = if w == 0 || h == 0 || binary_sauvola.len() < (w as usize) * (h as usize) {
        0
    } else {
        trace_boundary_loops(binary_sauvola, w, h)
            .iter()
            .filter(|lp| lp.len() >= 3)
            .map(|lp| lp.len() + 1) // M + (n-1)L + Z = n+1
            .sum()
    };
    let simplified: usize = vectorize_binary(binary_sauvola, w, h)
        .iter()
        .map(|p| p.len())
        .sum();
    let runlength: usize = vectorize_binary_runlength(binary_sauvola, w, h)
        .iter()
        .map(|p| p.len())
        .sum();
    (raw, simplified, runlength)
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
                // 輪郭方式（#112）とインポートフォントのパスで使う 3次ベジェ
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
    // TrueType と同じ nonzero winding で塗る（#112: 外輪郭 CW / 穴 CCW を前提に
    // 穴を抜き・交差ストロークを塗り残さない）。ランレングス方式の互いに素な矩形群でも
    // 同一結果になる。
    out.push_str(&format!(
        "  <path d=\"{d}\" fill=\"black\" fill-rule=\"nonzero\"/>\n"
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
    fn fixed_transform_real_dpi_crop_maps_within_8_units() {
        // 実DPI経路の回帰防止（#111 QA）: 本番の crop は mm_to_px(12mm).round() = 142px
        // （300dpi）で、理想 10px/mm のテストでは丸め経路を踏まない。
        // 内枠4辺（crop 内 1.0/11.0mm）をピクセルグリッドに丸めて置いた成分が、
        // 理想 em 座標（0 / 1000 / -120 / 880）から 8 units 以内に写ることを固定する
        let crop = layout::mm_to_px(layout::CELL_CROP_SIZE).round() as u32; // 142
        let to_px = |mm: f64| layout::mm_to_px(mm).round() as u32;
        // 内枠位置は crop 幾何の正本（inner_left_in_crop_mm = 1.0mm）から導出する
        let inner_left_mm = inner_left_in_crop_mm();
        let inner_bottom_mm = inner_left_mm + layout::INNER_SIZE;
        let x0 = to_px(inner_left_mm); // 内枠左端/上端
        let x1 = to_px(inner_bottom_mm); // 内枠右端/下端
        let binary = make_binary(crop, crop, &[(x0, x0, x1, x1)]);
        let (min_x, min_y, max_x, max_y) = paths_bbox(&vectorize_binary(&binary, crop, crop));
        assert!((min_x - 0.0).abs() <= 8.0, "内枠左端の写像誤差: {min_x}");
        assert!(
            (max_x - UNITS_PER_EM).abs() <= 8.0,
            "内枠右端の写像誤差: {max_x}"
        );
        assert!(
            (max_y - (layout::EMBOX_BOTTOM_Y + UNITS_PER_EM)).abs() <= 8.0,
            "内枠上端の写像誤差: {max_y}"
        );
        assert!(
            (min_y - layout::EMBOX_BOTTOM_Y).abs() <= 8.0,
            "内枠下端の写像誤差: {min_y}"
        );

        // ベースライン（内枠下端の GUIDE_BASELINE_OFFSET_MM 上）に下端が接する成分
        // → y=0 から 8 units 以内
        let yb = to_px(inner_bottom_mm - layout::GUIDE_BASELINE_OFFSET_MM);
        let binary = make_binary(crop, crop, &[(x0, yb - to_px(1.8), x1, yb)]);
        let (_, min_y, _, _) = paths_bbox(&vectorize_binary(&binary, crop, crop));
        assert!(min_y.abs() <= 8.0, "ベースラインの写像誤差: {min_y}");
    }

    #[test]
    fn guide_line_surviving_binarization_becomes_paths_without_review() {
        // 仕様文書化テスト（#111 QA、防御層デシジョンテーブル行5の固定）:
        // 全防御層（L1 シアン除去・L2 erase_grid_lines）を抜けて二値化まで生き残った
        // ガイド線は、セル crop の境界帯(2px)に接触しない（線の端点は crop 端から
        // 約1mm ≈ 11.8px 離れている）ため品質ゲート(#110)では原理的に検出できず、
        // needs_review も立たずに非空グリフとして混入する。これは現状の既知の限界。
        // 実運用の検知器はシアンサンプル未検出警告（remove_cyan → UI 警告）。
        // 将来ここに防御を追加したら、このテストの期待を反転させること。
        let crop = layout::mm_to_px(layout::CELL_CROP_SIZE).round() as u32;
        let to_px = |mm: f64| layout::mm_to_px(mm).round() as u32;
        // ガイド線の位置は crop 幾何の正本（inner_left_in_crop_mm）から導出する
        let inner_left_mm = inner_left_in_crop_mm();
        let inner_bottom_mm = inner_left_mm + layout::INNER_SIZE;
        let yb = to_px(inner_bottom_mm - layout::GUIDE_BASELINE_OFFSET_MM);
        let mut img = make_image(crop, crop, Rgba([255, 255, 255, 255]));
        // ベースラインガイド相当: 内枠幅いっぱい・太さ3px の黒線（モノクロ印刷の代理）
        for y in yb..yb + 3 {
            for x in to_px(inner_left_mm)..to_px(inner_bottom_mm) {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let (binary, quality) = binarize_with_quality(&img);
        assert!(
            !quality.needs_review,
            "境界非接触のガイド線では要確認が立たない（現状仕様）"
        );
        assert!(quality.kept_components >= 1, "ガイド線成分が生き残る（現状仕様）");
        let paths = vectorize_binary(&binary, crop, crop);
        assert!(!paths.is_empty(), "ガイド線が非空グリフとして混入する（現状仕様）");
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
    fn runlength_vertical_merge_reduces_rect_count() {
        // ランレングス方式（フォールバック）の縦マージ回帰: 縦10px×横40pxの縦棒。
        // 2x アップスケール後は80行のランが出るが、同一幅なので縦マージで大幅に減る。
        let binary = make_binary(100, 100, &[(45, 30, 55, 70)]);
        let paths = vectorize_binary_runlength(&binary, 100, 100);
        assert!(
            paths.len() < 20,
            "縦マージにより矩形数は大幅に減るはず: 実際={}",
            paths.len()
        );
        assert!(!paths.is_empty(), "黒ピクセルがあるのでパスは0にならない");
        // ランレングスの各パスは M→L→L→L→Z の四角形
        for p in &paths {
            assert_eq!(p.len(), 5, "ランレングスの各矩形は5コマンド");
        }
    }

    // ── 輪郭ベクター化（#112） ──
    //
    // 検証補助: パスの on-curve 頂点列（M/L/C の終点。制御点は除く）を多角形として扱う。

    fn on_curve_points(sub: &[PathCommand]) -> Vec<(f64, f64)> {
        let mut pts = Vec::new();
        for cmd in sub {
            match cmd {
                PathCommand::MoveTo { x, y } | PathCommand::LineTo { x, y } => pts.push((*x, *y)),
                PathCommand::CurveTo { x, y, .. } => pts.push((*x, *y)),
                PathCommand::Close { .. } => {}
            }
        }
        pts
    }

    /// サブパスの符号付き面積（font 空間 Y 上向き、shoelace）。
    /// 正 = CCW（穴）、負 = CW（外輪郭）。on-curve 頂点の多角形で近似する
    /// （ベジェ丸めはコーナー三角形内に収まるため符号は安定）。
    fn subpath_signed_area(sub: &[PathCommand]) -> f64 {
        let pts = on_curve_points(sub);
        let n = pts.len();
        if n < 3 {
            return 0.0;
        }
        let mut a = 0.0;
        for i in 0..n {
            let (x0, y0) = pts[i];
            let (x1, y1) = pts[(i + 1) % n];
            a += x0 * y1 - x1 * y0;
        }
        a / 2.0
    }

    /// nonzero winding number（全サブパスの on-curve 多角形で、点(px,py)を通る +x 方向
    /// レイの符号付き交差数）。TrueType の nonzero 塗りの判定に一致する。
    fn winding_number(paths: &[Vec<PathCommand>], px: f64, py: f64) -> i32 {
        let mut wn = 0i32;
        for sub in paths {
            let pts = on_curve_points(sub);
            let n = pts.len();
            if n < 3 {
                continue;
            }
            for i in 0..n {
                let (x0, y0) = pts[i];
                let (x1, y1) = pts[(i + 1) % n];
                if y0 <= py && y1 > py {
                    let t = (py - y0) / (y1 - y0);
                    if x0 + t * (x1 - x0) > px {
                        wn += 1; // 上向き交差
                    }
                } else if y1 <= py && y0 > py {
                    let t = (py - y0) / (y1 - y0);
                    if x0 + t * (x1 - x0) > px {
                        wn -= 1; // 下向き交差
                    }
                }
            }
        }
        wn
    }

    /// (w,h) の Sauvola バイナリを作り、black_rects を黒・white_rects を白で上書きする。
    /// 穴あき形状（アニュラス等）の合成に使う。
    fn make_shape(
        w: u32,
        h: u32,
        black_rects: &[(u32, u32, u32, u32)],
        white_rects: &[(u32, u32, u32, u32)],
    ) -> Vec<u8> {
        let mut buf = make_binary(w, h, black_rects);
        for &(x0, y0, x1, y1) in white_rects {
            for y in y0..y1 {
                for x in x0..x1 {
                    buf[(y * w + x) as usize] = 255;
                }
            }
        }
        buf
    }

    #[test]
    fn contour_solid_rect_is_cw_quadrilateral() {
        // 塗り矩形 → 単一輪郭・5コマンド（M,L,L,L,Z）・外輪郭は font 空間で CW（面積<0）
        let binary = make_binary(120, 120, &[(15, 85, 35, 105)]);
        let paths = vectorize_binary(&binary, 120, 120);
        assert_eq!(paths.len(), 1, "塗り矩形は輪郭1本");
        assert_eq!(paths[0].len(), 5, "矩形の角は保存され M,L,L,L,Z の5コマンド");
        assert!(
            subpath_signed_area(&paths[0]) < 0.0,
            "外輪郭は font 空間で CW（符号付き面積<0）"
        );
        // 塗り内部は nonzero で塗られる（winding != 0）
        let (min_x, min_y, max_x, max_y) = paths_bbox(&paths);
        let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        assert_ne!(winding_number(&paths, cx, cy), 0, "塗り矩形内部は塗られる");
    }

    #[test]
    fn contour_annulus_hole_stays_open() {
        // アニュラス（穴あき四角）: 外輪郭 CW・穴 CCW で、nonzero 塗りで穴が潰れない。
        // 穴あき文字「あ・お・ぬ・ふ・ぼ」が崩れない性質の合成再現（#112 最重要）。
        let binary = make_shape(120, 120, &[(20, 20, 100, 100)], &[(50, 50, 70, 70)]);
        let paths = vectorize_binary(&binary, 120, 120);
        assert_eq!(paths.len(), 2, "外輪郭 + 穴 = 2本");

        // 面積の大きい方が外輪郭、小さい方が穴。符号（巻き方向）は逆でなければならない
        let mut areas: Vec<f64> = paths.iter().map(|p| subpath_signed_area(p)).collect();
        areas.sort_by(|a, b| a.abs().partial_cmp(&b.abs()).unwrap());
        let (hole_area, outer_area) = (areas[0], areas[1]);
        assert!(outer_area < 0.0, "外輪郭は CW（面積<0）: {outer_area}");
        assert!(hole_area > 0.0, "穴は CCW（面積>0）: {hole_area}");

        // nonzero winding: 穴の中心は塗られない（wn==0）、ストローク帯は塗られる（wn!=0）
        let t = EmTransform::new(120, 120);
        let hole_center = (t.fx(60.0), t.fy(60.0));
        assert_eq!(
            winding_number(&paths, hole_center.0, hole_center.1),
            0,
            "穴の中心は nonzero で塗られない（穴が開いている）"
        );
        let stroke_pt = (t.fx(30.0), t.fy(30.0)); // 外輪郭内・穴の外
        assert_ne!(
            winding_number(&paths, stroke_pt.0, stroke_pt.1),
            0,
            "ストローク帯は塗られる"
        );
    }

    #[test]
    fn contour_two_holes_all_ccw() {
        // 穴2つ（「ぬ」等の複数ループ相当）→ 3本、外1本 CW・穴2本 CCW。
        let binary = make_shape(
            160,
            120,
            &[(20, 20, 140, 100)],
            &[(40, 45, 60, 75), (100, 45, 120, 75)],
        );
        let paths = vectorize_binary(&binary, 160, 120);
        assert_eq!(paths.len(), 3, "外輪郭 + 穴2 = 3本");
        let cw = paths.iter().filter(|p| subpath_signed_area(p) < 0.0).count();
        let ccw = paths.iter().filter(|p| subpath_signed_area(p) > 0.0).count();
        assert_eq!((cw, ccw), (1, 2), "外輪郭1(CW) + 穴2(CCW)");
        // 両方の穴の中心が塗られない
        let t = EmTransform::new(160, 120);
        for hx in [50.0, 110.0] {
            let wn = winding_number(&paths, t.fx(hx), t.fy(60.0));
            assert_eq!(wn, 0, "穴 x={hx} の中心は塗られない");
        }
    }

    #[test]
    fn contour_nested_island_fills() {
        // 穴の中の島（塗り）: 外輪郭CW → 穴CCW → 島CW。島の内側は再び塗られる。
        // 順序が要る（外→穴を白抜き→島を黒）ため make_shape でなく手で構築する
        let (w, h) = (160u32, 160u32);
        let mut binary = make_binary(w, h, &[(20, 20, 140, 140)]); // 外枠 黒
        for y in 45..115 {
            for x in 45..115 {
                binary[(y * w + x) as usize] = 255; // 島を囲む穴 白抜き
            }
        }
        for y in 65..95 {
            for x in 65..95 {
                binary[(y * w + x) as usize] = 0; // 中央の島 黒
            }
        }
        let paths = vectorize_binary(&binary, 160, 160);
        assert_eq!(paths.len(), 3, "外輪郭 + 穴 + 島 = 3本");
        let t = EmTransform::new(160, 160);
        // 島の中心（塗り）
        assert_ne!(winding_number(&paths, t.fx(80.0), t.fy(80.0)), 0, "島の内側は塗られる");
        // 穴の帯（島の外・穴の内）は塗られない
        assert_eq!(winding_number(&paths, t.fx(55.0), t.fy(55.0)), 0, "穴の帯は塗られない");
        // 外枠の帯（穴の外）は塗られる
        assert_ne!(winding_number(&paths, t.fx(30.0), t.fy(30.0)), 0, "外枠の帯は塗られる");
    }

    #[test]
    fn contour_winding_thin_line_no_flip() {
        // 巻き方向の符号境界（near-zero area）: 細い線（3px 幅・DP 許容誤差超）でも
        // 外輪郭 CW を保つ・空にならない・panic しない。
        for &(rect, w, h) in &[
            (&(50u32, 10u32, 53u32, 90u32), 100u32, 100u32), // 縦3px線
            (&(10, 50, 90, 53), 100, 100),                   // 横3px線
        ] {
            let binary = make_binary(w, h, &[*rect]);
            let paths = vectorize_binary(&binary, w, h);
            assert!(!paths.is_empty(), "細線でも空にならない: {rect:?}");
            for p in &paths {
                let a = subpath_signed_area(p);
                assert!(a < 0.0, "細線の外輪郭も CW（面積<0）: area={a}, rect={rect:?}");
            }
        }
    }

    #[test]
    fn contour_small_block_is_cw() {
        // 小さいが DP 許容誤差を上回る成分（6px 角）は CW 輪郭として残る
        let binary = make_binary(60, 60, &[(27, 27, 33, 33)]);
        let paths = vectorize_binary(&binary, 60, 60);
        assert_eq!(paths.len(), 1, "6px角は輪郭1本");
        assert!(subpath_signed_area(&paths[0]) < 0.0, "小成分の輪郭も CW");
    }

    #[test]
    fn contour_subepsilon_degenerates_to_empty_no_panic() {
        // 退化ケース: 単一画素・1px 幅の線は DP 許容誤差（1.5px）未満で畳まれ空になる。
        // panic せず穏当に劣化する（実運用では品質ゲートが <10px スペックを上流で除去）。
        for &(rect, w, h) in &[
            (&(30u32, 30u32, 31u32, 31u32), 60u32, 60u32), // 単一画素
            (&(50, 10, 51, 90), 100, 100),                 // 1px 縦線
        ] {
            let binary = make_binary(w, h, &[*rect]);
            let paths = vectorize_binary(&binary, w, h);
            assert!(paths.is_empty(), "サブ許容誤差の成分は空へ畳まれる: {rect:?} → {}", paths.len());
        }
    }

    #[test]
    fn contour_border_touching_closes() {
        // 画像端に接触する成分（画像外=背景なので端が境界になる）→ 閉輪郭・panic しない
        let binary = make_binary(80, 80, &[(0, 0, 40, 40)]);
        let paths = vectorize_binary(&binary, 80, 80);
        assert_eq!(paths.len(), 1);
        assert!(subpath_signed_area(&paths[0]) < 0.0, "端接触でも外輪郭 CW");
        // 左上端は塗られる
        let t = EmTransform::new(80, 80);
        assert_ne!(winding_number(&paths, t.fx(10.0), t.fy(10.0)), 0);
    }

    #[test]
    fn contour_full_black_cell_no_panic() {
        // 全面黒: 外周1本の輪郭。panic せず非空
        let binary = make_binary(64, 64, &[(0, 0, 64, 64)]);
        let paths = vectorize_binary(&binary, 64, 64);
        assert_eq!(paths.len(), 1, "全面黒はセル外周の輪郭1本");
        assert!(subpath_signed_area(&paths[0]) < 0.0);
    }

    #[test]
    fn contour_explosion_returns_empty() {
        // 輪郭本数の暴走（MAX_CONTOURS 超）→ 空へ倒す（ハングガード）。
        // 6px グリッドに 2px 角の孤立ブロックを敷き詰めて多数の独立輪郭を作る。
        let mut rects = Vec::new();
        for gy in 0..40u32 {
            for gx in 0..40u32 {
                let bx = gx * 6 + 1;
                let by = gy * 6 + 1;
                rects.push((bx, by, bx + 2, by + 2));
            }
        }
        assert!(rects.len() > MAX_CONTOURS, "テスト前提: 輪郭本数が上限超");
        let binary = make_binary(250, 250, &rects);
        let paths = vectorize_binary(&binary, 250, 250);
        assert!(paths.is_empty(), "輪郭爆発時はハングガードで空: 実際={}", paths.len());

        // #112: 二値化は非空（黒あり）なのに空へ倒れる = 黙って欠字。
        // vectorize_adopted_with_review 経由（本番と同一経路）で needs_review が立つこと。
        assert!(binary_has_ink(&binary, 250, 250), "テスト前提: 二値化は非空");
        let mut quality = CellQuality::empty();
        let paths2 = vectorize_adopted_with_review(&binary, 250, 250, &mut quality);
        assert!(paths2.is_empty(), "採用経路でも空に倒れる");
        assert!(quality.needs_review, "MAX_CONTOURS 発火時は needs_review が立つ（黙って欠字にしない）");
    }

    #[test]
    fn contour_reduces_commands_vs_runlength() {
        // 対角ストローク: 輪郭方式のコマンド数がランレングスより桁違いに少ない（#112 の目的）。
        let mut rects = Vec::new();
        for i in 0..80u32 {
            // 幅8pxの対角帯
            rects.push((10 + i, 10 + i, 18 + i, 18 + i));
        }
        let binary = make_binary(120, 120, &rects);
        let contour = vectorize_binary(&binary, 120, 120);
        let runlength = vectorize_binary_runlength(&binary, 120, 120);
        let ccmd: usize = contour.iter().map(|p| p.len()).sum();
        let rcmd: usize = runlength.iter().map(|p| p.len()).sum();
        assert!(!contour.is_empty());
        assert!(
            ccmd * 3 < rcmd,
            "輪郭方式のコマンド数はランレングスより大幅に少ないはず: contour={ccmd}, runlength={rcmd}"
        );
    }

    #[test]
    fn dp_epsilon_boundary_keeps_and_drops() {
        // Douglas-Peucker の許容誤差境界: 直線から距離 d の中間点は、eps<d で残り eps>d で消える。
        // 3点 (0,0)-(10,d)-(20,0)。中点の垂線距離 = d。
        let line = |d: i32| vec![(0i32, 0i32), (10, d), (20, 0)];
        // d=3。eps=2.0(<3) → 中点保持（3点）、eps=4.0(>3) → 中点除去（2点）
        assert_eq!(dp_open(&line(3), 2.0).len(), 3, "eps<d は中間点を残す");
        assert_eq!(dp_open(&line(3), 4.0).len(), 2, "eps>d は中間点を除去");
        // 使用中の CONTOUR_DP_EPSILON_PX 近傍: d=1(<eps=1.5)は消える、d=3(>eps)は残る
        assert_eq!(dp_open(&line(1), CONTOUR_DP_EPSILON_PX).len(), 2, "微小凹凸は畳まれる");
        assert_eq!(
            dp_open(&line(3), CONTOUR_DP_EPSILON_PX).len(),
            3,
            "有意な凹凸は保持される（過度に単純化しない）"
        );
    }

    #[test]
    fn dp_closed_preserves_rectangle_corners() {
        // 閉多角形 DP は矩形の4角を保持し collinear な辺上点を落とす（過小簡約しない下限）。
        let mut poly = Vec::new();
        for x in 0..=20 {
            poly.push((x, 0)); // 上辺
        }
        for y in 1..=10 {
            poly.push((20, y)); // 右辺
        }
        for x in (0..20).rev() {
            poly.push((x, 10)); // 下辺
        }
        for y in (1..10).rev() {
            poly.push((0, y)); // 左辺
        }
        let simplified = douglas_peucker_closed(&poly, CONTOUR_DP_EPSILON_PX);
        assert_eq!(simplified.len(), 4, "矩形は4角に簡約される: {simplified:?}");
        for corner in [(0, 0), (20, 0), (20, 10), (0, 10)] {
            assert!(simplified.contains(&corner), "角 {corner:?} が保持される");
        }
    }

    #[test]
    fn smooth_preserves_sharp_corner_rounds_gentle() {
        // 角保存ベジェ: 90°の鋭角は直線接続（L）で残り、緩い曲がりは3次ベジェ（C）で丸める。
        // 鋭角の L 字（90°の角）
        let sharp = vec![(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)];
        let sp = smooth_contour_to_path(&sharp);
        assert!(
            sp.iter().all(|c| !matches!(c, PathCommand::CurveTo { .. })),
            "90°の角はベジェにせず直線で残す: {sp:?}"
        );
        // 頂点(100,0) が LineTo として残る = コーナー保存
        assert!(
            sp.iter().any(|c| matches!(c, PathCommand::LineTo { x, y } if (*x-100.0).abs()<1.0 && y.abs()<1.0)),
            "鋭角の頂点そのものが保持される"
        );

        // 緩い曲がり（ほぼ直線＝小さいターン角）→ 丸める（C を含む）
        let gentle = vec![(0.0, 0.0), (100.0, 6.0), (200.0, 0.0)];
        let gp = smooth_contour_to_path(&gentle);
        assert!(
            gp.iter().any(|c| matches!(c, PathCommand::CurveTo { .. })),
            "緩い頂点は3次ベジェで丸める: {gp:?}"
        );
    }

    #[test]
    fn smooth_bezier_control_within_corner_triangle() {
        // ベジェ近似の誤差上限: 丸めの制御点は元の頂点、端点は隣接辺上でカット比率以内。
        // = 丸めがコーナー三角形に収まる（自己交差を生まない・過度に膨らまない）。
        let v = vec![(0.0, 0.0), (100.0, 10.0), (200.0, 0.0)];
        let path = smooth_contour_to_path(&v);
        let curve = path
            .iter()
            .find_map(|c| match c {
                PathCommand::CurveTo { x, y, cp1x, cp1y, cp2x, cp2y } => {
                    Some((*x, *y, *cp1x, *cp1y, *cp2x, *cp2y))
                }
                _ => None,
            })
            .expect("緩い頂点なので C があるはず");
        let (ex, ey, cp1x, cp1y, cp2x, cp2y) = curve;
        // 制御点は頂点(100,10)に一致（丸め = 頂点を制御点にした2次相当の3次）
        assert!((cp1x - 100.0).abs() < 1.5 && (cp1y - 10.0).abs() < 1.5, "cp1 は頂点");
        assert!((cp2x - 100.0).abs() < 1.5 && (cp2y - 10.0).abs() < 1.5, "cp2 は頂点");
        // 射出端点は頂点(100,10)→(200,0)辺上、カット比率(0.42)×辺長以内
        // 辺長 ≈ sqrt(100^2+10^2) ≈ 100.5、カット ≈ 42。端点は頂点から ~42 の位置
        let cut = ((ex - 100.0).powi(2) + (ey - 10.0).powi(2)).sqrt();
        assert!(cut <= 0.42 * 100.5 + 2.0, "射出カットは辺長×0.42以内: {cut}");
        assert!(cut > 10.0, "丸めが潰れず有効なカット長を持つ: {cut}");
    }

    // ── 追加テスト（#112 QA 不足分の補完） ──

    #[test]
    fn choose_next_pinch_selects_most_clockwise() {
        // ピンチ頂点（出口複数）: 入射方向 din=(1,0)（左→右）に対し、画像 Y 下向きで
        // 最も時計回り = 下向き(0,1) を選ぶ。上向き(0,-1) は最も反時計回りなので選ばない。
        let cur = (5, 5);
        let outs = vec![(5, 4), (5, 6)]; // (上=Y-1, 下=Y+1)
        let idx = choose_next(&outs, Some((1, 0)), cur);
        assert_eq!(outs[idx], (5, 6), "右進入では下向き出口（最も時計回り）を選ぶ");

        // 逆に din=(-1,0)（右→左）なら時計回りは上向き(0,-1)
        let idx2 = choose_next(&outs, Some((-1, 0)), cur);
        assert_eq!(outs[idx2], (5, 4), "左進入では上向き出口を選ぶ");

        // 単一出口 / din 無しは常に 0（一意）
        assert_eq!(choose_next(&[(1, 0)], None, cur), 0);
        assert_eq!(choose_next(&outs, None, cur), 0);
    }

    #[test]
    fn contour_two_px_line_winding_cw() {
        // 2px 幅の細線（DP 許容誤差 1.5px 超）: 縦横とも外輪郭 CW（面積<0）・空にならない・
        // 巻き方向が反転しない。3px 版（contour_winding_thin_line_no_flip）の下側境界。
        for &(rect, w, h) in &[
            (&(50u32, 10u32, 52u32, 90u32), 100u32, 100u32), // 縦2px線
            (&(10, 50, 90, 52), 100, 100),                   // 横2px線
        ] {
            let binary = make_binary(w, h, &[*rect]);
            let paths = vectorize_binary(&binary, w, h);
            assert!(!paths.is_empty(), "2px 細線でも空にならない: {rect:?}");
            for p in &paths {
                assert!(
                    subpath_signed_area(p) < 0.0,
                    "2px 細線の外輪郭も CW（面積<0）: rect={rect:?}"
                );
            }
        }
    }

    #[test]
    fn dp_epsilon_strict_boundary() {
        // DP の判定は `far_d2 > eps2`（厳密 >）。垂線距離 d がちょうど eps と等しいとき、
        // 点は「残らない」（境界は除去側）。3点 (0,0)-(10,d)-(20,0)、中点の距離=d。
        let line = |d: i32| vec![(0i32, 0i32), (10, d), (20, 0)];
        // d=3、eps=3.0 ちょうど: d2=9, eps2=9 → 9>9 は偽 → 中点除去（2点）
        assert_eq!(dp_open(&line(3), 3.0).len(), 2, "距離=eps ちょうどは除去（厳密 >）");
        // eps を僅かに下げる（2.999）と 9 > 8.994 で保持（3点）
        assert_eq!(dp_open(&line(3), 2.999).len(), 3, "eps を僅かに下回ると保持");
    }

    /// 孤立した cell_px 角ブロックを `count` 個、格子状に並べた二値化を作る（連結しない間隔）。
    /// 各ブロックが1本の独立輪郭になり、輪郭本数の境界テストに使う。
    fn grid_blocks(count: usize, cell_px: u32) -> (Vec<u8>, u32, u32) {
        const COLS: u32 = 40;
        const SP: u32 = 8; // ブロック間隔（cell_px より十分大きく、必ず非連結）
        let rows = (count as u32).div_ceil(COLS);
        let w = COLS * SP + SP;
        let h = rows * SP + SP;
        let mut rects = Vec::with_capacity(count);
        for k in 0..count as u32 {
            let (gx, gy) = (k % COLS, k / COLS);
            let (bx, by) = (gx * SP + 2, gy * SP + 2);
            rects.push((bx, by, bx + cell_px, by + cell_px));
        }
        (make_binary(w, h, &rects), w, h)
    }

    #[test]
    fn max_contours_lower_boundary() {
        // 輪郭本数の境界: ちょうど MAX_CONTOURS 本は通す（非空）、1本超で空へ倒す。
        // ガードは `loops.len() > MAX_CONTOURS`（厳密 >）。
        // cell_px=4: DP 許容誤差(1.5px)を角の逸脱(4/√2≈2.83px)が上回り、各ブロックが
        // 4角の輪郭として残る（2px だと角が畳まれて退化するため 4px を使う）。
        let (at_limit, w1, h1) = grid_blocks(MAX_CONTOURS, 4);
        let paths_at = vectorize_binary(&at_limit, w1, h1);
        assert_eq!(paths_at.len(), MAX_CONTOURS, "ちょうど上限本は全て残る");

        let (over, w2, h2) = grid_blocks(MAX_CONTOURS + 1, 4);
        let paths_over = vectorize_binary(&over, w2, h2);
        assert!(paths_over.is_empty(), "上限+1本で空へ倒す: 実際={}", paths_over.len());
    }

    #[test]
    fn max_contour_points_fires_and_flags_review() {
        // 総頂点数ガード: 輪郭本数は MAX_CONTOURS 以下でも、単純化後の総頂点が
        // MAX_CONTOUR_POINTS を超えたら空へ倒す。多数の「櫛」形状（1本で頂点多数）を
        // 縦に積み、頂点総数だけを膨らませる。
        const BANDS: u32 = 70;
        const BAND_H: u32 = 12; // bar(3) + tooth(6) + gap(3)
        let x_end = 902u32;
        let w = x_end + 2;
        let h = BANDS * BAND_H + 4;
        let mut rects: Vec<(u32, u32, u32, u32)> = Vec::new();
        for b in 0..BANDS {
            let y0 = 2 + b * BAND_H;
            // 上辺のバー（全櫛を1連結成分にする）
            rects.push((2, y0, x_end, y0 + 3));
            // 3px 幅・6px 長の歯を 6px ピッチで下げる（振幅6px > DP 許容誤差 → 全頂点が残る）
            let mut x = 2;
            while x + 3 < x_end {
                rects.push((x, y0 + 3, x + 3, y0 + 9));
                x += 6;
            }
        }
        let binary = make_binary(w, h, &rects);
        // 輪郭本数は BANDS 本（<= MAX_CONTOURS）だが総頂点は上限超で空へ倒れる
        assert!((BANDS as usize) <= MAX_CONTOURS, "テスト前提: 輪郭本数は上限以下");

        let mut quality = CellQuality::empty();
        let paths = vectorize_adopted_with_review(&binary, w, h, &mut quality);
        assert!(
            paths.is_empty(),
            "MAX_CONTOUR_POINTS 発火で空へ倒れるべき: 実際={}",
            paths.len()
        );
        assert!(binary_has_ink(&binary, w, h), "テスト前提: 二値化は非空");
        assert!(
            quality.needs_review,
            "MAX_CONTOUR_POINTS 発火時も needs_review が立つ（黙って欠字にしない）"
        );
    }

    #[test]
    fn adopted_cell_empty_after_pre_gate_erasure_flags_review() {
        // pre-gate 消失サブクラス（#112/#108 セルフレビュー指摘）:
        // 生セルにインクがあり judge_adoption が採用したセルでも、Sauvola 閾値処理や
        // morphological_open_close で品質ゲート到達**前**にストロークが消えると、
        // gated_binary は全白（インクなし）になる。ゲートは 0 成分除去なので単独では
        // needs_review を立てない。それでも採用セルがベクター化できない以上、
        // needs_review を立てなければ「黙って欠字」する。
        //
        // vectorize_adopted_with_review は採用セルにのみ呼ばれる前提なので、全白 gated を
        // 渡した時点で「採用されたが gated が空」= pre-gate 消失を表す。
        let gated_empty = vec![255u8; 100 * 100]; // 全白 = ゲート後インクなし
        assert!(
            !binary_has_ink(&gated_empty, 100, 100),
            "テスト前提: gated_binary はインクなし（pre-gate で消失した状態）"
        );
        let mut quality = CellQuality::empty();
        let paths = vectorize_adopted_with_review(&gated_empty, 100, 100, &mut quality);
        assert!(paths.is_empty(), "空 gated はパスなし");
        assert!(
            quality.needs_review,
            "採用セルがベクター化できないなら、pre-gate 消失（gated 空）でも needs_review が立つ"
        );
    }

    #[test]
    fn contour_multi_level_nesting_depth_ge_3() {
        // 多層内包（depth>=3）: 塗り→穴→島→島内の穴 の4重同心。
        // 巻き方向は CW→CCW→CW→CCW と交互になり、nonzero で塗り/空が交互になる。
        let (w, h) = (200u32, 200u32);
        let mut binary = make_binary(w, h, &[(10, 10, 190, 190)]); // L0 外枠 黒
        let fill = |buf: &mut [u8], x0, y0, x1, y1, v: u8| {
            for y in y0..y1 {
                for x in x0..x1 {
                    buf[(y * w + x) as usize] = v;
                }
            }
        };
        fill(&mut binary, 35, 35, 165, 165, 255); // L1 穴 白
        fill(&mut binary, 60, 60, 140, 140, 0); // L2 島 黒
        fill(&mut binary, 85, 85, 115, 115, 255); // L3 島内の穴 白

        let paths = vectorize_binary(&binary, w, h);
        assert_eq!(paths.len(), 4, "4重同心 → 輪郭4本");

        let t = EmTransform::new(w, h);
        // 各層の帯中央での nonzero winding: L0塗り, L1空, L2塗り, L3空
        assert_ne!(winding_number(&paths, t.fx(22.0), t.fy(22.0)), 0, "L0 外枠帯は塗り");
        assert_eq!(winding_number(&paths, t.fx(47.0), t.fy(47.0)), 0, "L1 穴帯は空");
        assert_ne!(winding_number(&paths, t.fx(72.0), t.fy(72.0)), 0, "L2 島帯は塗り");
        assert_eq!(winding_number(&paths, t.fx(100.0), t.fy(100.0)), 0, "L3 島内の穴は空");
    }

    #[test]
    fn corner_threshold_neighborhood_rounds_below_keeps_above() {
        // 角閾値（CONTOUR_CORNER_THRESHOLD_DEG=62°）近傍: ターン角が閾値未満なら3次ベジェで
        // 丸め（C を含む）、閾値超なら角として直線接続のみ（C を含まない）。
        // din=(+x)。頂点で角度 deg だけ曲げた3点多角形を作ると、その頂点のターン角=deg。
        let mk = |deg: f64| {
            let a = deg.to_radians();
            vec![
                (0.0, 0.0),
                (100.0, 0.0),
                (100.0 + 100.0 * a.cos(), 100.0 * a.sin()),
            ]
        };
        let below = smooth_contour_to_path(&mk(61.0));
        assert!(
            below.iter().any(|c| matches!(c, PathCommand::CurveTo { .. })),
            "閾値(62°)未満のターン角は丸める（C あり）: {below:?}"
        );
        let above = smooth_contour_to_path(&mk(63.0));
        assert!(
            above.iter().all(|c| !matches!(c, PathCommand::CurveTo { .. })),
            "閾値超のターン角は角として残す（C なし）: {above:?}"
        );
    }
}
