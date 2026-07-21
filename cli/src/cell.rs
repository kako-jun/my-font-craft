// セル切り出し + チェック欄解析 + 採用判定
use image::{RgbaImage, Rgba};
use serde::{Serialize, Deserialize};
use crate::layout;

#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// チェック欄の状態
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CheckMark {
    Check,  // ✓
    Empty,  // 空欄
}

/// 1マスの解析結果
#[derive(Debug, Serialize, Deserialize)]
pub struct SlotResult {
    pub cell_index: usize,   // 0=左, 1=右
    pub is_empty: bool,
    pub black_ratio: f64,
    pub check_mark: CheckMark,
    pub check_density: f64,  // チェック欄の黒ピクセル密度（デバッグ用）
}

/// 1文字（2マス）の採用判定結果
#[derive(Debug, Serialize, Deserialize)]
pub struct CharResult {
    pub row: usize,
    pub col: usize,
    pub slots: [SlotResult; 2],
    pub adopted: Vec<usize>,     // 採用されたセルインデックス
    pub adoption_reason: String, // 採用理由（デバッグ用）
}

/// CLI用: 全48文字を処理してファイルに保存
#[cfg(not(target_arch = "wasm32"))]
pub fn extract_and_judge(img: &RgbaImage, output_dir: &Path) -> Result<Vec<CharResult>, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("セル出力ディレクトリ作成エラー: {e}"))?;

    let border_margin = 1.0;
    let crop_size = layout::CELL_SIZE - border_margin * 2.0;
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

    let mut results = Vec::new();
    let mut total_adopted = 0usize;
    let mut total_empty = 0usize;

    // 輪郭ベクター化の cmd/glyph 計測（#112）: 非空グリフの (単純化前, 単純化後, ランレングス)
    let mut cmd_glyphs = 0usize;
    let mut cmd_raw_sum = 0usize;
    let mut cmd_simplified_sum = 0usize;
    let mut cmd_runlength_sum = 0usize;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }
            let mut slots = Vec::new();

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
                let cell_img = crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px);

                let black_ratio = measure_inner_black_ratio(&cell_img, 0.2);
                let is_empty = black_ratio < 0.02;

                let check_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let check_px_y = layout::mm_to_px(mm_y + layout::CELL_SIZE + border_margin * 0.5).round() as u32;
                let check_w = layout::mm_to_px(layout::CELL_SIZE - border_margin * 2.0).round() as u32;
                let check_h = layout::mm_to_px(layout::CHECK_HEIGHT - border_margin).round() as u32;
                let check_img = crop_region(img, check_px_x, check_px_y, check_w, check_h);

                let (check_mark, check_density) = analyze_check_mark(&check_img);

                // ベクター化入力は WASM 経路（pipeline.rs）と同じ標準 crop
                // （CELL_CROP_MARGIN=1.5mm、12mm四方）を使う。セル→em 固定変換（#111）は
                // この物理寸法を前提にしているため、解析用 crop（1.0mm）と分ける
                let vec_cell = extract_cell_image_raw(img, row, col, cell_idx);

                // 生セル画像（ベクター化入力と同じ crop）
                let filename_raw = format!("R{row:02}C{col:02}_I{cell_idx}_raw.png");
                vec_cell.save(output_dir.join(&filename_raw))
                    .map_err(|e| format!("セル保存エラー {filename_raw}: {e}"))?;

                // 二値化+品質ゲート（#110）はセルごとに1回だけ実行し、
                // プレビュー画像とベクター化の両方に同じバイナリを使い回す（pipeline.rs と同じ流儀）
                let (gated_binary, quality) =
                    crate::vectorizer::binarize_with_quality(&vec_cell);
                let binarized = crate::vectorizer::binary_to_rgba(
                    &gated_binary,
                    vec_cell.width(),
                    vec_cell.height(),
                );
                let filename = format!("R{row:02}C{col:02}_I{cell_idx}.png");
                binarized.save(output_dir.join(&filename))
                    .map_err(|e| format!("セル保存エラー {filename}: {e}"))?;

                // ベジェパス（JSON + SVG）
                let paths = crate::vectorizer::vectorize_binary(
                    &gated_binary,
                    vec_cell.width(),
                    vec_cell.height(),
                );
                let json = serde_json::to_string_pretty(&paths)
                    .map_err(|e| format!("paths JSONシリアライズエラー: {e}"))?;
                let json_filename = format!("R{row:02}C{col:02}_I{cell_idx}_paths.json");
                std::fs::write(output_dir.join(&json_filename), json)
                    .map_err(|e| format!("paths JSON保存エラー {json_filename}: {e}"))?;

                let svg = crate::vectorizer::paths_to_svg(&paths);
                let svg_filename = format!("R{row:02}C{col:02}_I{cell_idx}_paths.svg");
                std::fs::write(output_dir.join(&svg_filename), svg)
                    .map_err(|e| format!("paths SVG保存エラー {svg_filename}: {e}"))?;

                // ジャギー比較用にランレングス方式（フォールバック）の SVG も並置出力（#112）
                let rl_paths = crate::vectorizer::vectorize_binary_runlength(
                    &gated_binary,
                    vec_cell.width(),
                    vec_cell.height(),
                );
                let rl_svg = crate::vectorizer::paths_to_svg(&rl_paths);
                let rl_svg_filename = format!("R{row:02}C{col:02}_I{cell_idx}_paths_runlength.svg");
                std::fs::write(output_dir.join(&rl_svg_filename), rl_svg)
                    .map_err(|e| format!("runlength SVG保存エラー {rl_svg_filename}: {e}"))?;

                // cmd/glyph 3点計測（#112）: 非空グリフのみ集計
                if !paths.is_empty() {
                    let (raw, simplified, runlength) = crate::vectorizer::vectorize_command_counts(
                        &gated_binary,
                        vec_cell.width(),
                        vec_cell.height(),
                    );
                    cmd_glyphs += 1;
                    cmd_raw_sum += raw;
                    cmd_simplified_sum += simplified;
                    cmd_runlength_sum += runlength;
                    log!(
                        "  R{row:02}C{col:02}_I{cell_idx} cmd: 単純化前={raw} 輪郭={simplified} ランレングス={runlength}"
                    );
                }

                let check_filename = format!("R{row:02}C{col:02}_I{cell_idx}_check.png");
                check_img.save(output_dir.join(&check_filename))
                    .map_err(|e| format!("チェック欄保存エラー {check_filename}: {e}"))?;

                log!(
                    "  R{row:02}C{col:02}_I{cell_idx}: black={:.1}% {} check={:?}({:.1}%)",
                    black_ratio * 100.0,
                    if is_empty { "空" } else { "非空" },
                    check_mark,
                    check_density * 100.0,
                );

                if quality.needs_review {
                    log!(
                        "  ⚠ R{row:02}C{col:02}_I{cell_idx}: 品質ゲート要確認 (removed={}, removed_area={:.2}%, kept={}, ink={:.1}%)",
                        quality.removed_components,
                        quality.removed_area_ratio * 100.0,
                        quality.kept_components,
                        quality.ink_ratio * 100.0,
                    );
                }

                slots.push(SlotResult {
                    cell_index: cell_idx,
                    is_empty,
                    black_ratio,
                    check_mark,
                    check_density,
                });
            }

            let (adopted, reason) = judge_adoption(&slots);

            if adopted.is_empty() {
                total_empty += 1;
            } else {
                total_adopted += 1;
            }

            if !adopted.is_empty() || slots.iter().any(|s| !s.is_empty) {
                log!(
                    "  → R{row:02}C{col:02} 採用: {:?} ({})",
                    adopted, reason
                );
            }

            let slots_arr = [slots.remove(0), slots.remove(0)];
            results.push(CharResult {
                row,
                col,
                slots: slots_arr,
                adopted,
                adoption_reason: reason,
            });
        }
    }

    log!("\n  文字サマリー: 採用={total_adopted}, 空={total_empty}, 合計={}", results.len());
    if cmd_glyphs > 0 {
        let n = cmd_glyphs as f64;
        log!(
            "  cmd/glyph 平均（非空{cmd_glyphs}字）: 単純化前={:.1} 輪郭={:.1} ランレングス={:.1}（削減 {:.1}x）",
            cmd_raw_sum as f64 / n,
            cmd_simplified_sum as f64 / n,
            cmd_runlength_sum as f64 / n,
            cmd_runlength_sum as f64 / cmd_simplified_sum.max(1) as f64,
        );
    }
    Ok(results)
}

/// WASM用: 全48文字を処理（ファイル保存なし）
pub fn extract_and_judge_in_memory(img: &RgbaImage) -> Result<Vec<CharResult>, String> {
    let border_margin = 1.0;
    let crop_size = layout::CELL_SIZE - border_margin * 2.0;
    let crop_size_px = layout::mm_to_px(crop_size).round() as u32;

    let mut results = Vec::new();
    let mut total_adopted = 0usize;
    let mut total_empty = 0usize;

    for row in 0..layout::ROWS {
        for col in 0..layout::COLS {
            if layout::is_skipped_cell(row, col) {
                continue;
            }
            let mut slots = Vec::new();

            for cell_idx in 0..2 {
                let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_idx);

                let crop_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let crop_px_y = layout::mm_to_px(mm_y + border_margin).round() as u32;
                let cell_img = crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px);

                let black_ratio = measure_inner_black_ratio(&cell_img, 0.2);
                let is_empty = black_ratio < 0.02;

                let check_px_x = layout::mm_to_px(mm_x + border_margin).round() as u32;
                let check_px_y = layout::mm_to_px(mm_y + layout::CELL_SIZE + border_margin * 0.5).round() as u32;
                let check_w = layout::mm_to_px(layout::CELL_SIZE - border_margin * 2.0).round() as u32;
                let check_h = layout::mm_to_px(layout::CHECK_HEIGHT - border_margin).round() as u32;
                let check_img = crop_region(img, check_px_x, check_px_y, check_w, check_h);

                let (check_mark, check_density) = analyze_check_mark(&check_img);

                log!(
                    "  R{row:02}C{col:02}_I{cell_idx}: black={:.1}% {} check={:?}({:.1}%)",
                    black_ratio * 100.0,
                    if is_empty { "空" } else { "非空" },
                    check_mark,
                    check_density * 100.0,
                );

                slots.push(SlotResult {
                    cell_index: cell_idx,
                    is_empty,
                    black_ratio,
                    check_mark,
                    check_density,
                });
            }

            let (adopted, reason) = judge_adoption(&slots);

            if adopted.is_empty() {
                total_empty += 1;
            } else {
                total_adopted += 1;
            }

            if !adopted.is_empty() || slots.iter().any(|s| !s.is_empty) {
                log!(
                    "  → R{row:02}C{col:02} 採用: {:?} ({})",
                    adopted, reason
                );
            }

            let slots_arr = [slots.remove(0), slots.remove(0)];
            results.push(CharResult {
                row,
                col,
                slots: slots_arr,
                adopted,
                adoption_reason: reason,
            });
        }
    }

    log!("  文字サマリー: 採用={total_adopted}, 空={total_empty}, 合計={}", results.len());
    Ok(results)
}

/// セル画像を切り出して返す（生RGBA、ベクター化前の内部処理用）
///
/// マージンは layout::CELL_CROP_MARGIN（#34 で 1.5mm。1mm だと台形補正+TPS 後の
/// 残差ズレで下端に外枠線が写り込むケースがあった。INNER_SIZE=10mm に対して
/// まだ 1mm の余裕がある）。この crop の物理寸法（CELL_CROP_SIZE=12mm 四方）が
/// vectorizer.rs のセル→em 固定変換（#111）の前提なので、変更は必ず layout の
/// 定数を通すこと。
pub fn extract_cell_image_raw(img: &RgbaImage, row: usize, col: usize, cell_index: usize) -> RgbaImage {
    let crop_size_px = layout::mm_to_px(layout::CELL_CROP_SIZE).round() as u32;

    let (mm_x, mm_y) = layout::get_cell_position(row, col, cell_index);
    let crop_px_x = layout::mm_to_px(mm_x + layout::CELL_CROP_MARGIN).round() as u32;
    let crop_px_y = layout::mm_to_px(mm_y + layout::CELL_CROP_MARGIN).round() as u32;
    crop_region(img, crop_px_x, crop_px_y, crop_size_px, crop_size_px)
}

/// セル画像を切り出して二値化済み（白背景+黒ストローク）RGBA として返す
/// JS プレビューと Rust のベクター化が同じ入力を使えるようにする
pub fn extract_cell_image(img: &RgbaImage, row: usize, col: usize, cell_index: usize) -> RgbaImage {
    let raw = extract_cell_image_raw(img, row, col, cell_index);
    crate::vectorizer::binarize_to_rgba(&raw)
}

/// 採用判定: docs/template-spec.md の採用ルール
///
/// 1. 両方空欄 → 採用なし
/// 2. 左のみ記入 → I0 採用
/// 3. 右のみ記入 → I1 採用
/// 4. 両方記入 かつ どちらにも✓なし → I1 採用
/// 5. 両方記入 かつ 両方に✓ → I1 採用
/// 6. 両方記入 かつ 片方だけ✓ → ✓のある方を採用
fn judge_adoption(slots: &[SlotResult]) -> (Vec<usize>, String) {
    // 記入済み（非空）マスを抽出
    let filled: Vec<usize> = slots.iter()
        .filter(|s| !s.is_empty)
        .map(|s| s.cell_index)
        .collect();

    if filled.is_empty() {
        return (vec![], "両方空".to_string());
    }

    if filled.len() == 1 {
        let idx = filled[0];
        return (vec![idx], format!("片方のみ記入 → I{idx}を採用"));
    }

    // 両方記入済み。✓付きマスを数える
    let checked: Vec<usize> = slots.iter()
        .filter(|s| !s.is_empty && s.check_mark == CheckMark::Check)
        .map(|s| s.cell_index)
        .collect();

    if checked.len() == 1 {
        let idx = checked[0];
        return (vec![idx], format!("片方だけ✓ → I{idx}を採用"));
    }

    // 両方✓ or 両方無印 → 右(I1)を採用
    let reason = if checked.len() == 2 {
        "両方✓ → 右(I1)を採用".to_string()
    } else {
        "✓なし → 右(I1)を採用".to_string()
    };
    (vec![1], reason)
}

/// チェック欄の解析: 黒ピクセル密度で ✓/空欄 を判定
/// Sauvola適応的二値化で黒ピクセルを判定
fn analyze_check_mark(check_img: &RgbaImage) -> (CheckMark, f64) {
    let w = check_img.width();
    let h = check_img.height();
    if w == 0 || h == 0 {
        return (CheckMark::Empty, 0.0);
    }

    let gray = rgba_to_gray(check_img);
    let gray = if detect_moire(&gray, w, h) {
        median_filter_3x3(&gray, w, h)
    } else {
        gray
    };
    let gray = apply_clahe(&gray, w, h);
    let binary = binarize_hybrid(&gray, w, h);
    let binary = morphological_open_close(&binary, w, h);

    let total = w * h;
    let black_count = binary.iter().filter(|&&v| v == 0).count() as u32;
    let density = black_count as f64 / total as f64;

    // 閾値:
    // - 2%未満: 空欄（ノイズや格子線の残骸）
    // - 2%以上: ✓（記入あり）
    let mark = if density < 0.02 {
        CheckMark::Empty
    } else {
        CheckMark::Check
    };

    (mark, density)
}

/// 内側領域の黒ピクセル率を計測（Sauvola適応的二値化）
fn measure_inner_black_ratio(img: &RgbaImage, margin_ratio: f64) -> f64 {
    let w = img.width();
    let h = img.height();
    let margin_x = (w as f64 * margin_ratio).round() as u32;
    let margin_y = (h as f64 * margin_ratio).round() as u32;

    let end_x = w.saturating_sub(margin_x);
    let end_y = h.saturating_sub(margin_y);
    if margin_x >= end_x || margin_y >= end_y {
        return 0.0;
    }

    // 画像全体でSauvola二値化（局所的な閾値を正しく計算するため）
    let gray = rgba_to_gray(img);
    let gray = if detect_moire(&gray, w, h) {
        median_filter_3x3(&gray, w, h)
    } else {
        gray
    };
    let gray = apply_clahe(&gray, w, h);
    let binary = binarize_hybrid(&gray, w, h);
    let mut binary = morphological_open_close(&binary, w, h);
    // 孤立スペック除去: モルフォロジを生き残った小さな黒ブロブ（シアン残骸・点ノイズ）を消す。
    // これがないと数px角の残骸が複数あるだけで空欄セルが「非空」と誤判定され、空欄を黒グリフ化しうる。
    // 手書きストロークは連結成分が大きいので MIN_SPECK_AREA 程度では消えない（薄い細線でも面積は十分大きい）。
    remove_small_black_components(&mut binary, w, h, MIN_SPECK_AREA);

    // 内側領域のみカウント
    let mut black_count = 0u32;
    let mut total = 0u32;

    for y in margin_y..end_y {
        for x in margin_x..end_x {
            total += 1;
            if binary[(y * w + x) as usize] == 0 {
                black_count += 1;
            }
        }
    }

    if total > 0 {
        black_count as f64 / total as f64
    } else {
        0.0
    }
}

// ── セル品質ゲート（#110: 連結成分分析による枠残渣の決定論的除去） ──

/// 品質ゲートの境界帯の幅（px）。
/// モルフォロジ Closing の最終 Erosion は「画像外=白」扱いのため、二値化後の
/// 最外周1pxは常に白になる。境界から侵入した枠・罫線残渣は必ず2px目以降に
/// 残るので、帯を2pxにして「境界接触」を判定する。
const GATE_BORDER_BAND: u32 = 2;

/// 境界接触成分を「はみ出した手書きストローク」とみなして保護する面積比の下限。
/// セル全体に対する成分の占有率がこれ以上なら、除去せず needs_review だけ立てる。
/// 手書き文字の連結成分は太く面積が大きい（典型でセルの5〜15%）のに対し、
/// erase_grid_lines をすり抜けた枠・罫線残渣は細線で 1〜3% 程度に収まる。
const GATE_STROKE_PROTECT_AREA_RATIO: f64 = 0.04;

/// 境界接触成分を「罫線・枠線」とみなす bbox の最小辺の上限（px）。
/// バウンディングボックスの短辺がこれ以下の成分は面積に関わらず線残渣として
/// 除去する。手書きペンのストロークは 300DPI で 5px 以上の太さがあり、
/// はみ出し文字は成分全体（文字1字）の bbox が正方形に近くなるため誤爆しない。
const GATE_LINE_MAX_THICKNESS: u32 = 3;

/// needs_review を立てる除去面積比（除去した黒画素 / セル全画素）。
/// 内側の微小スペック除去は日常的に起きる正常動作なので、この閾値を超える
/// 「まとまった量の除去」だけを要確認にする。
const GATE_REVIEW_REMOVED_AREA_RATIO: f64 = 0.01;

/// 品質ゲートの結果（wasm 出力 JSON に載せて scanner / review UI へ伝搬する）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellQuality {
    /// 除去した連結成分の数（境界接触 + 微小スペック）
    pub removed_components: usize,
    /// 除去した黒画素のセル全画素に対する比率
    pub removed_area_ratio: f64,
    /// ゲート通過後に残った黒連結成分の数
    pub kept_components: usize,
    /// ゲート通過後のインク率（黒画素 / セル全画素）。
    /// ゲートはインクブリード補正（1px erosion）より前に走るため、これは補正**前**の計測値
    pub ink_ratio: f64,
    /// 要確認フラグ。真なら review UI で「要確認」として見せる（黙って空に倒さない）
    pub needs_review: bool,
}

impl CellQuality {
    /// ゲート対象がない空入力用のデフォルト
    pub fn empty() -> Self {
        Self {
            removed_components: 0,
            removed_area_ratio: 0.0,
            kept_components: 0,
            ink_ratio: 0.0,
            needs_review: false,
        }
    }
}

/// セル品質ゲート: 二値化・モルフォロジ後の binary（0=黒/255=白）に対して
/// 8近傍ラベリングで黒連結成分を列挙し、以下を適用する。
///
/// 1. **境界接触成分の除去** — セル外周 GATE_BORDER_BAND px の帯に触れる成分は
///    枠・罫線残渣とみなして白に倒す。ただし「はみ出して書いた字」を消さない
///    安全弁として、面積比 >= GATE_STROKE_PROTECT_AREA_RATIO かつ bbox 短辺 >
///    GATE_LINE_MAX_THICKNESS の成分はストロークと判定して残し、needs_review だけ立てる。
///    帯内でも面積 < MIN_SPECK_AREA の微小成分はスペックノイズ扱いに降格して除去し、
///    それ単独では needs_review を立てない（エッジのダストでの偽陽性を防ぐ）。
/// 2. **面積フィルタ** — 境界に触れない成分でも面積 < MIN_SPECK_AREA は
///    スペックノイズとして除去（remove_small_black_components の一般化）。
/// 3. **品質スコア** — 除去数・除去面積比・残成分数・インク率から needs_review を決める。
///    条件（保守的）: 境界接触除去が発生 / 保護したはみ出しストロークがある /
///    除去面積比 > GATE_REVIEW_REMOVED_AREA_RATIO / 除去の結果 残成分がゼロ化。
pub fn apply_cell_quality_gate(binary: &mut [u8], w: u32, h: u32) -> CellQuality {
    let n = (w as usize) * (h as usize);
    if n == 0 || binary.len() < n {
        return CellQuality::empty();
    }

    let total = n as f64;
    let mut visited = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut comp: Vec<usize> = Vec::new();

    let mut removed_components = 0usize;
    let mut removed_area = 0usize;
    let mut kept_components = 0usize;
    let mut kept_area = 0usize;
    let mut removed_border = 0usize;
    let mut protected_border_stroke = false;

    for start in 0..n {
        if visited[start] || binary[start] != 0 {
            continue;
        }
        // 8近傍で連結成分を収集
        comp.clear();
        stack.clear();
        stack.push(start);
        visited[start] = true;

        let mut touches_border = false;
        let (mut bx_min, mut bx_max, mut by_min, mut by_max) = (u32::MAX, 0u32, u32::MAX, 0u32);

        while let Some(idx) = stack.pop() {
            comp.push(idx);
            let x = (idx as u32) % w;
            let y = (idx as u32) / w;
            if x < GATE_BORDER_BAND
                || y < GATE_BORDER_BAND
                || x >= w.saturating_sub(GATE_BORDER_BAND)
                || y >= h.saturating_sub(GATE_BORDER_BAND)
            {
                touches_border = true;
            }
            bx_min = bx_min.min(x);
            bx_max = bx_max.max(x);
            by_min = by_min.min(y);
            by_max = by_max.max(y);

            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x as i64 + dx;
                    let ny = y as i64 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let ni = (ny as u32 * w + nx as u32) as usize;
                    if !visited[ni] && binary[ni] == 0 {
                        visited[ni] = true;
                        stack.push(ni);
                    }
                }
            }
        }

        let area = comp.len();
        let area_ratio = area as f64 / total;
        let bbox_min_dim = (bx_max - bx_min + 1).min(by_max - by_min + 1);

        let remove = if touches_border {
            if (area as u32) < MIN_SPECK_AREA {
                // 偽陽性の降格: 帯内でも微小成分はスペックノイズ扱いで除去する。
                // removed_border に数えない = それ単独では needs_review を立てない
                // （エッジの1pxダスト程度でセルが警告になるオオカミ少年化を防ぐ）
                true
            } else {
                let is_line = bbox_min_dim <= GATE_LINE_MAX_THICKNESS;
                let is_big = area_ratio >= GATE_STROKE_PROTECT_AREA_RATIO;
                if is_big && !is_line {
                    // 安全弁: はみ出して書いた字の可能性 → 消さずに要確認だけ立てる
                    protected_border_stroke = true;
                    false
                } else {
                    removed_border += 1;
                    true
                }
            }
        } else {
            // 面積フィルタ（境界非接触）: スペックノイズのみ除去
            (area as u32) < MIN_SPECK_AREA
        };

        if remove {
            removed_components += 1;
            removed_area += area;
            for &idx in &comp {
                binary[idx] = 255;
            }
        } else {
            kept_components += 1;
            kept_area += area;
        }
    }

    let removed_area_ratio = removed_area as f64 / total;
    let zeroed = removed_components > 0 && kept_components == 0;
    let needs_review = removed_border > 0
        || protected_border_stroke
        || removed_area_ratio > GATE_REVIEW_REMOVED_AREA_RATIO
        || zeroed;

    CellQuality {
        removed_components,
        removed_area_ratio,
        kept_components,
        // 注意: ゲートはインクブリード補正（1px erosion）より前に走るため、
        // ink_ratio は補正前の計測値（最終グリフのインク率より少し大きめに出る）
        ink_ratio: kept_area as f64 / total,
        needs_review,
    }
}

/// 空欄判定用スペック除去のしきい値（連結成分の面積、px）。
/// 想定する残骸は 3px 角程度（面積 9）のシアン点ノイズなので、面積 9 以下（< 10）だけを消す。
/// 必要十分な最小値に絞ることで、かすれた細線が断片化しても消し過ぎないようにする。
/// 手書きの細線は連結成分が長く面積が十分大きいため、この値では消えない。
const MIN_SPECK_AREA: u32 = 10;

/// 面積が min_area 未満の黒連結成分（4近傍）を白で塗りつぶす。
/// binary は 0=黒(前景) / 非0=白(背景) の規約。
fn remove_small_black_components(binary: &mut [u8], w: u32, h: u32, min_area: u32) {
    let n = (w as usize) * (h as usize);
    if binary.len() < n {
        return;
    }
    let mut visited = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut comp: Vec<usize> = Vec::new();

    for start in 0..n {
        if visited[start] || binary[start] != 0 {
            continue;
        }
        comp.clear();
        stack.clear();
        stack.push(start);
        visited[start] = true;

        while let Some(idx) = stack.pop() {
            comp.push(idx);
            let x = (idx as u32) % w;
            let y = (idx as u32) / w;
            if x > 0 {
                let ni = idx - 1;
                if !visited[ni] && binary[ni] == 0 {
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
            if x + 1 < w {
                let ni = idx + 1;
                if !visited[ni] && binary[ni] == 0 {
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
            if y > 0 {
                let ni = idx - w as usize;
                if !visited[ni] && binary[ni] == 0 {
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
            if y + 1 < h {
                let ni = idx + w as usize;
                if !visited[ni] && binary[ni] == 0 {
                    visited[ni] = true;
                    stack.push(ni);
                }
            }
        }

        if (comp.len() as u32) < min_area {
            for &idx in &comp {
                binary[idx] = 255; // 白に倒す
            }
        }
    }
}

// ── CLAHE（ローカルコントラスト正規化） ──

const CLAHE_GRID: u32 = 4;   // タイル分割数
const CLAHE_CLIP: f64 = 3.0; // クリッピング係数

/// グレースケール画像にCLAHE（Contrast Limited Adaptive Histogram Equalization）を適用
fn apply_clahe(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    if w == 0 || h == 0 {
        return vec![];
    }

    let grid = CLAHE_GRID;
    let tile_w = w / grid;
    let tile_h = h / grid;

    if tile_w == 0 || tile_h == 0 {
        return gray.to_vec();
    }

    // 各タイルのLUT（ルックアップテーブル）を計算
    let mut luts = vec![[0u8; 256]; (grid * grid) as usize];

    for ty in 0..grid {
        for tx in 0..grid {
            let x0 = tx * tile_w;
            let y0 = ty * tile_h;
            let x1 = if tx == grid - 1 { w } else { x0 + tile_w };
            let y1 = if ty == grid - 1 { h } else { y0 + tile_h };

            // ヒストグラム計算
            let mut hist = [0u32; 256];
            let mut pixel_count = 0u32;
            for y in y0..y1 {
                for x in x0..x1 {
                    hist[gray[(y * w + x) as usize] as usize] += 1;
                    pixel_count += 1;
                }
            }

            if pixel_count == 0 {
                continue;
            }

            // クリッピング
            let clip_limit = (CLAHE_CLIP * pixel_count as f64 / 256.0).max(1.0) as u32;
            let mut excess = 0u32;
            for bin in hist.iter_mut() {
                if *bin > clip_limit {
                    excess += *bin - clip_limit;
                    *bin = clip_limit;
                }
            }
            // 超過分を均等再配分
            let per_bin = excess / 256;
            let remainder = (excess % 256) as usize;
            for (i, bin) in hist.iter_mut().enumerate() {
                *bin += per_bin;
                if i < remainder {
                    *bin += 1;
                }
            }

            // CDF計算 → LUT生成
            let mut cdf = [0u32; 256];
            cdf[0] = hist[0];
            for i in 1..256 {
                cdf[i] = cdf[i - 1] + hist[i];
            }
            let cdf_min = cdf.iter().copied().find(|&v| v > 0).unwrap_or(0);
            let denom = pixel_count.saturating_sub(cdf_min);

            let lut = &mut luts[(ty * grid + tx) as usize];
            for i in 0..256 {
                if denom == 0 {
                    lut[i] = i as u8;
                } else {
                    lut[i] = ((cdf[i].saturating_sub(cdf_min) as f64 * 255.0 / denom as f64)
                        .round()
                        .clamp(0.0, 255.0)) as u8;
                }
            }
        }
    }

    // バイリニア補間で最終出力を生成
    let mut out = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let val = gray[(y * w + x) as usize] as usize;

            // タイル中心座標を基準にした相対位置を計算
            // タイル中心: (tx + 0.5) * tile_w
            let fx = (x as f64 / tile_w as f64) - 0.5;
            let fy = (y as f64 / tile_h as f64) - 0.5;

            let tx0 = (fx.floor() as i32).clamp(0, grid as i32 - 1) as u32;
            let ty0 = (fy.floor() as i32).clamp(0, grid as i32 - 1) as u32;
            let tx1 = (tx0 + 1).min(grid - 1);
            let ty1 = (ty0 + 1).min(grid - 1);

            let ax = (fx - fx.floor()).clamp(0.0, 1.0);
            let ay = (fy - fy.floor()).clamp(0.0, 1.0);

            let v00 = luts[(ty0 * grid + tx0) as usize][val] as f64;
            let v10 = luts[(ty0 * grid + tx1) as usize][val] as f64;
            let v01 = luts[(ty1 * grid + tx0) as usize][val] as f64;
            let v11 = luts[(ty1 * grid + tx1) as usize][val] as f64;

            let top = v00 * (1.0 - ax) + v10 * ax;
            let bot = v01 * (1.0 - ax) + v11 * ax;
            let result = top * (1.0 - ay) + bot * ay;

            out[(y * w + x) as usize] = result.round().clamp(0.0, 255.0) as u8;
        }
    }

    out
}

// ── Sauvola 適応的二値化 ──

/// Sauvola法パラメータ
const SAUVOLA_K: f64 = 0.2;       // 感度パラメータ（文書画像の標準値）
const SAUVOLA_WINDOW: u32 = 15;   // 局所ウィンドウの一辺（300DPI、セル153×153px程度に適切）

/// RGBA画像をグレースケール配列に変換
fn rgba_to_gray(img: &RgbaImage) -> Vec<u8> {
    let w = img.width();
    let h = img.height();
    let mut gray = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let p = img.get_pixel(x, y);
            let lum = (p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114) as u8;
            gray.push(lum);
        }
    }
    gray
}

/// Sauvola法による適応的二値化
/// Integral Image（累積和テーブル）で局所平均・分散をO(1)計算
/// 閾値: T = mean * (1 + k * (std_dev / R - 1))
fn sauvola_binarize(gray: &[u8], w: u32, h: u32, k: f64, window_size: u32) -> Vec<u8> {
    let r_const = 128.0;
    let half = (window_size / 2) as i32;

    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }

    // Integral Image（累積和）と Integral Image^2（二乗累積和）を構築
    let mut sum = vec![0i64; n];
    let mut sq_sum = vec![0i64; n];

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let val = gray[idx] as i64;
            let val_sq = val * val;

            let left = if x > 0 { sum[idx - 1] } else { 0 };
            let up = if y > 0 { sum[((y - 1) * w + x) as usize] } else { 0 };
            let diag = if x > 0 && y > 0 { sum[((y - 1) * w + x - 1) as usize] } else { 0 };
            sum[idx] = val + left + up - diag;

            let left_sq = if x > 0 { sq_sum[idx - 1] } else { 0 };
            let up_sq = if y > 0 { sq_sum[((y - 1) * w + x) as usize] } else { 0 };
            let diag_sq = if x > 0 && y > 0 { sq_sum[((y - 1) * w + x - 1) as usize] } else { 0 };
            sq_sum[idx] = val_sq + left_sq + up_sq - diag_sq;
        }
    }

    // 各ピクセルの閾値を計算
    let mut binary = vec![255u8; n]; // デフォルト白

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let x0 = (x - half).max(0);
            let y0 = (y - half).max(0);
            let x1 = (x + half).min(w as i32 - 1);
            let y1 = (y + half).min(h as i32 - 1);

            let count = ((x1 - x0 + 1) * (y1 - y0 + 1)) as f64;

            let s = rect_sum(&sum, w, x0, y0, x1, y1) as f64;
            let s2 = rect_sum(&sq_sum, w, x0, y0, x1, y1) as f64;

            let mean = s / count;
            let variance = (s2 / count - mean * mean).max(0.0);
            let std_dev = variance.sqrt();

            // Sauvola閾値: T = mean * (1 + k * (std/R - 1))
            let threshold = mean * (1.0 + k * (std_dev / r_const - 1.0));

            let idx = (y as u32 * w + x as u32) as usize;
            let val = gray[idx] as f64;
            if val < threshold {
                binary[idx] = 0; // 黒
            }
        }
    }

    binary
}

// ── ハイブリッド二値化（#136） ──

/// グローバル閾値（紙白正規化済み画像が前提）。輝度がこの値未満ならインクとみなす。
///
/// 較正値 180: 実写真（筆ペン, IMG_20260720_162145.jpg, session #136）で
/// 太い筆ペンストローク内部（輝度おおよそ20〜60）を確実にインク判定しつつ、
/// 印刷ガイド線の薄グレー残骸（輝度およそ200台後半〜白に近い、erase_grid_lines
/// 後段で大半は消えているが取りこぼしが輝度220以上に残る想定）を拾わない値として選定。
/// Sauvola単独の弱点（窓幅を超える太いストロークで局所コントラストが消え内部を
/// 背景と誤判定する = 空洞化）を、紙白正規化済みという前提を活かした固定閾値で補う。
const GLOBAL_INK_THRESHOLD: u8 = 180;

/// グローバル閾値二値化: 輝度 < threshold を黒（インク）、それ以外を白とする単純な二値化。
fn global_binarize(gray: &[u8], threshold: u8) -> Vec<u8> {
    gray.iter()
        .map(|&v| if v < threshold { 0 } else { 255 })
        .collect()
}

/// ハイブリッド二値化（#136）: グローバル閾値とSauvola局所閾値の論理和（OR）。
///
/// 背景: 筆ペン（線幅2〜4mm ≒ 300DPIで24〜48px）は SAUVOLA_WINDOW(15px) より太く、
/// 窓内がインクだけで埋まると局所分散が0近くまで落ち Sauvola 閾値が引きずられて
/// ストローク内部を「背景」と誤判定する（穴あき文字になる）。
///
/// 対策: 紙白正規化済み画像に対する固定グローバル閾値（GLOBAL_INK_THRESHOLD）を
/// 追加し、「どちらかがインク判定なら黒」の OR で合成する。
/// - グローバル閾値: 太い濃いストロークの内部を面で確実に拾う（Sauvolaが取りこぼす領域）
/// - Sauvola: 薄い鉛筆・かすれた線の縁など、グローバル閾値では拾えない低コントラスト部を担当
///
/// 結果としてどちらか一方が単独で持つ弱点を他方が補う。
pub(crate) fn binarize_hybrid(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    let global = global_binarize(gray, GLOBAL_INK_THRESHOLD);
    let sauvola = sauvola_binarize(gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
    global
        .iter()
        .zip(sauvola.iter())
        .map(|(&g, &s)| if g == 0 || s == 0 { 0 } else { 255 })
        .collect()
}

/// Integral Imageから矩形領域の合計値を取得
fn rect_sum(integral: &[i64], w: u32, x0: i32, y0: i32, x1: i32, y1: i32) -> i64 {
    let w = w as i32;
    let br = integral[(y1 * w + x1) as usize];
    let tl = if x0 > 0 && y0 > 0 { integral[((y0 - 1) * w + x0 - 1) as usize] } else { 0 };
    let top = if y0 > 0 { integral[((y0 - 1) * w + x1) as usize] } else { 0 };
    let left = if x0 > 0 { integral[(y1 * w + x0 - 1) as usize] } else { 0 };
    br + tl - top - left
}

/// 領域切り出し
fn crop_region(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut out = RgbaImage::new(w, h);
    for dy in 0..h {
        for dx in 0..w {
            let sx = x + dx;
            let sy = y + dy;
            if sx < img.width() && sy < img.height() {
                out.put_pixel(dx, dy, *img.get_pixel(sx, sy));
            } else {
                out.put_pixel(dx, dy, Rgba([255, 255, 255, 255]));
            }
        }
    }
    out
}

// ── モルフォロジカル処理 ──

/// 3×3カーネルのErosion（収縮）: 近傍に白(255)があれば白にする
/// 黒領域を収縮させ、孤立黒ノイズを除去する
fn morphological_erode(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }
    let mut out = vec![0u8; n];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut has_white = false;
            'kernel: for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        // 画像外は白とみなす
                        has_white = true;
                        break 'kernel;
                    }
                    if binary[(ny as u32 * w + nx as u32) as usize] == 255 {
                        has_white = true;
                        break 'kernel;
                    }
                }
            }
            let idx = (y as u32 * w + x as u32) as usize;
            out[idx] = if has_white { 255 } else { 0 };
        }
    }
    out
}

/// 3×3カーネルのDilation（膨張）: 近傍に黒(0)があれば黒にする
/// 黒領域を膨張させ、孤立白ノイズを埋める
fn morphological_dilate(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    let n = (w * h) as usize;
    if n == 0 {
        return vec![];
    }
    let mut out = vec![255u8; n];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let mut has_black = false;
            'kernel: for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = x + dx;
                    let ny = y + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    if binary[(ny as u32 * w + nx as u32) as usize] == 0 {
                        has_black = true;
                        break 'kernel;
                    }
                }
            }
            let idx = (y as u32 * w + x as u32) as usize;
            out[idx] = if has_black { 0 } else { 255 };
        }
    }
    out
}

// ── vectorizer モジュール向けの公開再エクスポート ──
// cell.rs の内部ヘルパは pub(crate) だが、vectorizer から同一パイプラインで
// 呼びたいので薄いラッパを用意する。
pub(crate) fn rgba_to_gray_pub(img: &RgbaImage) -> Vec<u8> {
    rgba_to_gray(img)
}
pub(crate) fn apply_clahe_pub(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    apply_clahe(gray, w, h)
}
/// #136: セル二値化経路（vectorizer::binarize_with_quality）向けのハイブリッド二値化。
pub(crate) fn binarize_hybrid_pub(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    binarize_hybrid(gray, w, h)
}

/// Opening(Erode→Dilate)→Closing(Dilate→Erode)の一連処理
/// Opening: 小さな黒ノイズを除去、Closing: 小さな白ノイズを埋める
pub(crate) fn morphological_open_close(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    // Opening: Erode → Dilate
    let opened = morphological_dilate(&morphological_erode(binary, w, h), w, h);
    // Closing: Dilate → Erode
    morphological_erode(&morphological_dilate(&opened, w, h), w, h)
}

/// インクブリード補正（#57）: 印刷+スキャンでストロークが肥大した分を 1px erosion で戻す。
///
/// バリデーション前提: 手書きの細画を潰さないよう、erosion で黒ピクセルが 50% 未満に
/// 減ったらストロークが細すぎる（1px幅）と判断して補正を無効化する。
pub(crate) fn compensate_ink_bleed(binary: &[u8], w: u32, h: u32) -> Vec<u8> {
    // binary は Sauvola 出力形式（0=黒, 255=白）
    let before_black = binary.iter().filter(|&&v| v == 0).count();
    if before_black < 8 {
        // ほぼ空白セル: 補正しても無意味
        return binary.to_vec();
    }
    let eroded = morphological_erode(binary, w, h);
    let after_black = eroded.iter().filter(|&&v| v == 0).count();
    if (after_black as f64) / (before_black as f64) < 0.5 {
        // 細すぎるストローク保護: 補正をキャンセル
        return binary.to_vec();
    }
    eroded
}

// ── モアレパターン検出・除去 ──

/// ラプラシアンフィルタで高周波成分を抽出し、分散が閾値以上ならモアレありと判定
fn detect_moire(gray: &[u8], w: u32, h: u32) -> bool {
    if w < 3 || h < 3 {
        return false;
    }
    let w = w as usize;
    let h = h as usize;

    // ラプラシアンカーネル: [[0,-1,0],[-1,4,-1],[0,-1,0]]
    let mut sum: f64 = 0.0;
    let mut sum_sq: f64 = 0.0;
    let count = ((w - 2) * (h - 2)) as f64;

    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = gray[y * w + x] as f64;
            let top = gray[(y - 1) * w + x] as f64;
            let bottom = gray[(y + 1) * w + x] as f64;
            let left = gray[y * w + (x - 1)] as f64;
            let right = gray[y * w + (x + 1)] as f64;
            let lap = 4.0 * center - top - bottom - left - right;
            sum += lap;
            sum_sq += lap * lap;
        }
    }

    if count <= 0.0 {
        return false;
    }

    let mean = sum / count;
    let variance = sum_sq / count - mean * mean;
    variance >= 500.0
}

/// 3×3メディアンフィルタ。境界では利用可能なピクセルのみで計算
fn median_filter_3x3(gray: &[u8], w: u32, h: u32) -> Vec<u8> {
    if w == 0 || h == 0 {
        return vec![];
    }
    let w = w as usize;
    let h = h as usize;
    let mut out = vec![0u8; w * h];

    for y in 0..h {
        for x in 0..w {
            let mut neighbors = Vec::with_capacity(9);
            for dy in 0..3usize {
                let ny = y + dy;
                if ny < 1 || ny - 1 >= h {
                    continue;
                }
                let ny = ny - 1;
                for dx in 0..3usize {
                    let nx = x + dx;
                    if nx < 1 || nx - 1 >= w {
                        continue;
                    }
                    let nx = nx - 1;
                    neighbors.push(gray[ny * w + nx]);
                }
            }
            neighbors.sort_unstable();
            out[y * w + x] = neighbors[neighbors.len() / 2];
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ヘルパー ──

    fn make_slot(cell_index: usize, is_empty: bool, check_mark: CheckMark) -> SlotResult {
        SlotResult {
            cell_index,
            is_empty,
            black_ratio: if is_empty { 0.0 } else { 0.5 },
            check_mark,
            check_density: 0.0,
        }
    }

    // ── インクブリード補正のテスト (#57) ──

    #[test]
    fn compensate_ink_bleed_shrinks_thick_stroke() {
        // 12×12 に 7×7 の太めストロークを置くと erosion で 5×5 (25/49=51%) まで縮小。
        // 50% 以上残るので補正が適用される。
        let w = 12u32;
        let h = 12u32;
        let mut binary = vec![255u8; (w * h) as usize];
        for y in 2..9 {
            for x in 2..9 {
                binary[(y * w + x) as usize] = 0;
            }
        }
        let before_black = binary.iter().filter(|&&v| v == 0).count();
        let out = compensate_ink_bleed(&binary, w, h);
        let after_black = out.iter().filter(|&&v| v == 0).count();
        assert!(after_black < before_black, "太いストロークは erosion で縮小するはず");
        assert_eq!(after_black, 25, "7x7 の 1px erosion 後は 5x5 = 25px");
    }

    #[test]
    fn compensate_ink_bleed_preserves_thin_stroke() {
        // 1px 幅の縦棒は erosion でほぼ消える → 50% ガードで補正キャンセル、原形が維持される
        let w = 9u32;
        let h = 9u32;
        let mut binary = vec![255u8; (w * h) as usize];
        for y in 1..8 {
            binary[(y * w + 4) as usize] = 0;
        }
        let original = binary.clone();
        let out = compensate_ink_bleed(&binary, w, h);
        assert_eq!(out, original, "細すぎるストロークは補正キャンセルで原形保持");
    }

    #[test]
    fn compensate_ink_bleed_ignores_empty_cell() {
        // 空白セル（黒<8）は早期リターンで無変更
        let w = 9u32;
        let h = 9u32;
        let mut binary = vec![255u8; (w * h) as usize];
        binary[0] = 0; // 1ピクセルだけ黒
        let original = binary.clone();
        let out = compensate_ink_bleed(&binary, w, h);
        assert_eq!(out, original, "ほぼ空白のセルは補正しない");
    }

    fn make_uniform_image(w: u32, h: u32, color: Rgba<u8>) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, color);
            }
        }
        img
    }

    // ── judge_adoption: 新6ルール（template-spec.md 採用ルール表） ──

    #[test]
    fn judge_both_filled_both_check_adopts_right() {
        // I0記入✓, I1記入✓ → I1採用（両方✓は右優先）
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, false, CheckMark::Check),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_i0_check_i1_empty_mark() {
        // I0記入✓, I1記入空欄 → I0採用（片方だけ✓）
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_both_filled_i0_empty_mark_i1_check() {
        // I0記入空欄, I1記入✓ → I1採用（片方だけ✓）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, false, CheckMark::Check),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_both_filled_both_empty_mark() {
        // I0記入空欄, I1記入空欄 → I1採用（✓なし → 右優先）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_i0_filled_i1_empty_no_mark() {
        // I0記入空欄, I1空 → I0採用（唯一の記入済み）
        let slots = [
            make_slot(0, false, CheckMark::Empty),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_i0_empty_i1_filled_no_mark() {
        // I0空, I1記入空欄 → I1採用（唯一の記入済み）
        let slots = [
            make_slot(0, true, CheckMark::Empty),
            make_slot(1, false, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![1]);
    }

    #[test]
    fn judge_i0_filled_check_i1_empty() {
        // I0記入✓, I1空 → I0採用
        let slots = [
            make_slot(0, false, CheckMark::Check),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert_eq!(adopted, vec![0]);
    }

    #[test]
    fn judge_both_empty() {
        // I0空, I1空 → 採用なし（文字未記入）
        let slots = [
            make_slot(0, true, CheckMark::Empty),
            make_slot(1, true, CheckMark::Empty),
        ];
        let (adopted, _) = judge_adoption(&slots);
        assert!(adopted.is_empty());
    }

    // ── analyze_check_mark: 閾値境界テスト ──

    #[test]
    fn check_mark_empty_for_white_image() {
        let img = make_uniform_image(50, 20, Rgba([255, 255, 255, 255]));
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Empty);
        assert!(density < 0.02, "density={density} should be < 0.02");
    }

    #[test]
    fn check_mark_check_for_sparse_black() {
        // 密度5%程度 → Check（2%以上で Check）
        // モルフォロジカル処理に耐えるよう、3px以上の太さのブロックを配置
        let mut img = make_uniform_image(100, 100, Rgba([255, 255, 255, 255]));
        let total = 100 * 100;
        let target_black = (total as f64 * 0.05) as usize;
        // 5×5ブロックを複数配置（各25px、20ブロックで500px = 5%）
        let mut count = 0usize;
        'outer: for by in 0..4u32 {
            for bx in 0..5u32 {
                if count >= target_black { break 'outer; }
                for dy in 0..5u32 {
                    for dx in 0..5u32 {
                        let x = bx * 20 + dx + 2;
                        let y = by * 20 + dy + 2;
                        if x < 100 && y < 100 {
                            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                            count += 1;
                        }
                    }
                }
            }
        }
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Check);
        assert!(density >= 0.02, "density={density} should be >= 0.02");
    }

    #[test]
    fn check_mark_empty_for_zero_size() {
        let img = RgbaImage::new(0, 0);
        let (mark, density) = analyze_check_mark(&img);
        assert_eq!(mark, CheckMark::Empty);
        assert_eq!(density, 0.0);
    }

    // ── remove_small_black_components ──

    #[test]
    fn remove_small_components_drops_specks_keeps_strokes() {
        // 20x20 白(255)背景に、2x2 の小スペック(面積4)と 6x6 のブロック(面積36)を置く。
        // MIN_SPECK_AREA=10 未満の小スペックだけ消え、大きいブロックは残るべき。
        let w = 20u32;
        let h = 20u32;
        let mut binary = vec![255u8; (w * h) as usize];
        // スペック: (1,1)..(3,3)
        for y in 1..3 {
            for x in 1..3 {
                binary[(y * w + x) as usize] = 0;
            }
        }
        // ブロック: (10,10)..(16,16)
        for y in 10..16 {
            for x in 10..16 {
                binary[(y * w + x) as usize] = 0;
            }
        }
        remove_small_black_components(&mut binary, w, h, MIN_SPECK_AREA);

        // スペックは消えている
        assert_eq!(binary[(w + 1) as usize], 255, "面積4のスペックは消えるべき");
        // ブロックは残っている
        assert_eq!(binary[(12 * w + 12) as usize], 0, "面積36のブロックは残るべき");
    }

    #[test]
    fn remove_small_components_keeps_thin_stroke() {
        // かすれ細線の代理: 1px 幅・長さ15 の横線（面積15）。MIN_SPECK_AREA=10 では消えないこと。
        // 手書きストロークが断片化しても連結していれば残る、を保証するための回帰テスト。
        let w = 20u32;
        let h = 20u32;
        let mut binary = vec![255u8; (w * h) as usize];
        for x in 2..17 {
            binary[(10 * w + x) as usize] = 0;
        }
        remove_small_black_components(&mut binary, w, h, MIN_SPECK_AREA);
        assert_eq!(binary[(10 * w + 9) as usize], 0, "面積15の細線は残るべき");
    }

    // ── apply_cell_quality_gate（#110: セル品質ゲート） ──

    /// w×h の白バイナリ（255）を作る
    fn white_binary(w: u32, h: u32) -> Vec<u8> {
        vec![255u8; (w * h) as usize]
    }

    /// 矩形領域を黒(0)で塗る
    fn fill_black(binary: &mut [u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) {
        for y in y0..y1 {
            for x in x0..x1 {
                binary[(y * w + x) as usize] = 0;
            }
        }
    }

    #[test]
    fn gate_removes_border_touching_thin_line_keeps_stroke() {
        // 境界帯(2px)に接触する細い線（罫線残渣の代理）は除去され、
        // 内側の太いストロークは無傷で残る。needs_review が立つ。
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // 罫線残渣: y=1..3 の横線（厚み2px ≤ GATE_LINE_MAX_THICKNESS、境界帯に接触）
        fill_black(&mut binary, w, 0, 1, 40, 3);
        // ストローク: 内側の 8x8 ブロック
        fill_black(&mut binary, w, 15, 15, 23, 23);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[(2 * w + 20) as usize], 255, "境界接触の線残渣は消えるべき");
        assert_eq!(binary[(18 * w + 18) as usize], 0, "内側ストロークは残るべき");
        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 1);
        assert!(q.needs_review, "境界接触除去が発生したら要確認");
        assert!((q.ink_ratio - 64.0 / 1600.0).abs() < 1e-9);
    }

    #[test]
    fn gate_removes_corner_l_shaped_residue_by_area_rule() {
        // L字型の枠残渣（bbox は正方形に近いので thin ルールに掛からない）でも、
        // 面積比が保護閾値未満なら境界接触ルールで除去される。
        let w = 100u32;
        let h = 100u32;
        let mut binary = white_binary(w, h);
        // 上辺 + 左辺の L 字（2px厚、腕60px。面積 236/10000 = 2.4% < 4%）
        fill_black(&mut binary, w, 0, 0, 60, 2);
        fill_black(&mut binary, w, 0, 2, 2, 60);
        // 内側ストローク
        fill_black(&mut binary, w, 40, 40, 60, 60);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[(w + 30) as usize], 255, "L字残渣（横腕）は消えるべき");
        assert_eq!(binary[(30 * w + 1) as usize], 255, "L字残渣（縦腕）は消えるべき");
        assert_eq!(binary[(50 * w + 50) as usize], 0, "内側ストロークは残るべき");
        assert_eq!(q.removed_components, 1);
        assert!(q.needs_review);
    }

    #[test]
    fn gate_protects_overflowing_stroke() {
        // はみ出して書いた字の代理: 境界に接触するが太く面積の大きい成分は
        // 除去されず、needs_review だけ立つ（最悪の退行 = 字が丸ごと消える の安全弁）。
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // 左境界に接触する 20x30 ブロック（面積 600/1600 = 37.5% ≥ 4%、短辺 20 > 3）
        fill_black(&mut binary, w, 0, 5, 20, 35);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[(20 * w + 10) as usize], 0, "はみ出しストロークは残るべき");
        assert_eq!(q.removed_components, 0);
        assert_eq!(q.kept_components, 1);
        assert!(q.needs_review, "境界接触ストロークを保護したら要確認");
    }

    #[test]
    fn gate_removes_interior_speck_without_review() {
        // 内側の微小スペック除去（面積フィルタ）は日常動作なので needs_review は立たない
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // スペック: 2x2（面積4 < MIN_SPECK_AREA）
        fill_black(&mut binary, w, 10, 10, 12, 12);
        // ストローク: 8x8
        fill_black(&mut binary, w, 20, 20, 28, 28);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[(10 * w + 10) as usize], 255, "微小スペックは消えるべき");
        assert_eq!(binary[(24 * w + 24) as usize], 0, "ストロークは残るべき");
        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 1);
        assert!(!q.needs_review, "内側スペック除去だけなら要確認にしない");
    }

    #[test]
    fn gate_flags_zeroed_cell() {
        // 除去の結果、残成分がゼロ化したセルは needs_review（黙って空に倒さない）
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // 内側の微小スペックのみ（境界非接触）
        fill_black(&mut binary, w, 10, 10, 12, 12);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 0);
        assert!(q.needs_review, "残成分ゼロ化は要確認");
    }

    #[test]
    fn gate_clean_cell_untouched() {
        // 正常セル（内側ストロークのみ）は無変更・要確認なし
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        fill_black(&mut binary, w, 10, 10, 30, 30);
        let original = binary.clone();

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary, original, "正常セルは無変更");
        assert_eq!(q.removed_components, 0);
        assert_eq!(q.kept_components, 1);
        assert!(!q.needs_review);
        assert!((q.ink_ratio - 400.0 / 1600.0).abs() < 1e-9);
    }

    #[test]
    fn gate_empty_input() {
        let mut binary: Vec<u8> = vec![];
        let q = apply_cell_quality_gate(&mut binary, 0, 0);
        assert_eq!(q.removed_components, 0);
        assert_eq!(q.kept_components, 0);
        assert!(!q.needs_review);
    }

    // ── QA境界値テスト（#110） ──

    #[test]
    fn gate_band_boundary_x2_kept_x1_removed() {
        // 帯は外周2px（x<2 が帯内）。最近接画素 x=2 の成分（面積≥10）は帯外で放置、
        // x=1 まで達する同形の成分は帯内接触で除去される。
        let w = 40u32;
        let h = 40u32;

        // x=2..5（最近接 x=2 = 帯外）、面積 3x10=30
        let mut outside = white_binary(w, h);
        fill_black(&mut outside, w, 2, 10, 5, 20);
        let q1 = apply_cell_quality_gate(&mut outside, w, h);
        assert_eq!(outside[(15 * w + 3) as usize], 0, "x=2 の成分は帯外なので残るべき");
        assert_eq!(q1.removed_components, 0);
        assert!(!q1.needs_review);

        // x=1..4（最近接 x=1 = 帯内）、面積 3x10=30（比率1.9% < 4%）
        let mut inside = white_binary(w, h);
        fill_black(&mut inside, w, 1, 10, 4, 20);
        let q2 = apply_cell_quality_gate(&mut inside, w, h);
        assert_eq!(inside[(15 * w + 2) as usize], 255, "x=1 の成分は帯内接触で消えるべき");
        assert_eq!(q2.removed_components, 1);
        assert!(q2.needs_review, "境界接触除去は要確認");
    }

    #[test]
    fn gate_thin_rule_wins_over_big_protection() {
        // is_big（面積比≥4%）でも短辺 ≤ GATE_LINE_MAX_THICKNESS(3) なら線残渣として除去。
        // 短辺4なら保護。優先順位（is_big && !is_line のときだけ保護）を固定する。
        let w = 40u32;
        let h = 40u32;

        // 厚み3・長さ30 の境界接触線: 面積90 (5.6% ≥ 4%) だが短辺3 → 除去
        let mut thin = white_binary(w, h);
        fill_black(&mut thin, w, 0, 0, 30, 3);
        let q1 = apply_cell_quality_gate(&mut thin, w, h);
        assert_eq!(thin[(w + 15) as usize], 255, "短辺3の太幅線は面積が大きくても消えるべき");
        assert_eq!(q1.removed_components, 1);
        assert!(q1.needs_review);

        // 厚み4・長さ25 の境界接触ブロック: 面積100 (6.25%)・短辺4 → 保護
        let mut thick = white_binary(w, h);
        fill_black(&mut thick, w, 0, 0, 25, 4);
        let q2 = apply_cell_quality_gate(&mut thick, w, h);
        assert_eq!(thick[(2 * w + 15) as usize], 0, "短辺4かつ面積比≥4%は保護されるべき");
        assert_eq!(q2.removed_components, 0);
        assert_eq!(q2.kept_components, 1);
        assert!(q2.needs_review, "はみ出しストローク保護は要確認");
    }

    #[test]
    fn gate_area_ratio_exact_4pct_protected_below_removed() {
        // 40x40（総画素1600）: 面積比の保護判定は >= なので、64px（ちょうど4%）は保護、
        // 63px（4%未満）は除去。どちらも短辺 > 3 で thin ルールには掛からない。
        let w = 40u32;
        let h = 40u32;

        // 8x8 = 64px = 4.0% ちょうど、左境界に接触 → 保護
        let mut exact = white_binary(w, h);
        fill_black(&mut exact, w, 0, 10, 8, 18);
        let q1 = apply_cell_quality_gate(&mut exact, w, h);
        assert_eq!(exact[(14 * w + 4) as usize], 0, "面積比ちょうど4%は保護されるべき");
        assert_eq!(q1.removed_components, 0);
        assert!(q1.needs_review);

        // 9x7 = 63px < 4%、左境界に接触 → 除去
        let mut below = white_binary(w, h);
        fill_black(&mut below, w, 0, 10, 9, 17);
        let q2 = apply_cell_quality_gate(&mut below, w, h);
        assert_eq!(below[(13 * w + 4) as usize], 255, "面積比4%未満の境界接触成分は消えるべき");
        assert_eq!(q2.removed_components, 1);
        assert!(q2.needs_review);
    }

    #[test]
    fn gate_interior_area9_removed_area10_kept() {
        // 面積フィルタの境界値: 内側成分は面積9（< MIN_SPECK_AREA=10）で除去、10で残す
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // 3x3 = 9px の内側スペック
        fill_black(&mut binary, w, 10, 10, 13, 13);
        // 2x5 = 10px の内側成分
        fill_black(&mut binary, w, 20, 20, 22, 25);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[(11 * w + 11) as usize], 255, "面積9は消えるべき");
        assert_eq!(binary[(22 * w + 21) as usize], 0, "面積10は残るべき");
        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 1);
        assert!(!q.needs_review, "面積9の内側スペック除去（0.56% < 1%）は要確認なし");
    }

    #[test]
    fn gate_review_by_removed_area_ratio_threshold() {
        // 条件③単独（境界除去なし・保護なし・ゼロ化なし）: 内側スペックの除去合計が
        // 総画素の 1% を超えたときだけ needs_review（判定は厳密な >）。
        let w = 40u32;
        let h = 40u32;

        // 3x3 スペック ×2 = 18px (1.125% > 1%) + 残る内側ブロック → review
        let mut over = white_binary(w, h);
        fill_black(&mut over, w, 4, 4, 7, 7);
        fill_black(&mut over, w, 30, 4, 33, 7);
        fill_black(&mut over, w, 15, 15, 25, 25);
        let q1 = apply_cell_quality_gate(&mut over, w, h);
        assert_eq!(q1.removed_components, 2);
        assert_eq!(q1.kept_components, 1);
        assert!(q1.needs_review, "除去面積比 1.125% > 1% は要確認");

        // 2x4 スペック ×2 = 16px (ちょうど 1.0%、> でないので発火しない) → review なし
        let mut exact = white_binary(w, h);
        fill_black(&mut exact, w, 4, 4, 6, 8);
        fill_black(&mut exact, w, 30, 4, 32, 8);
        fill_black(&mut exact, w, 15, 15, 25, 25);
        let q2 = apply_cell_quality_gate(&mut exact, w, h);
        assert_eq!(q2.removed_components, 2);
        assert!((q2.removed_area_ratio - 0.01).abs() < 1e-9);
        assert!(!q2.needs_review, "除去面積比ちょうど1%は要確認なし");
    }

    #[test]
    fn gate_border_band_speck_demoted_no_review() {
        // 偽陽性の降格（QA修正B）: 帯内でも面積 < MIN_SPECK_AREA の微小成分は
        // スペック扱いで除去され、それ単独では needs_review を立てない。
        let w = 40u32;
        let h = 40u32;
        let mut binary = white_binary(w, h);
        // 帯内（角）の 2x2 ダスト
        fill_black(&mut binary, w, 0, 0, 2, 2);
        // 残る内側ストローク
        fill_black(&mut binary, w, 15, 15, 25, 25);

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert_eq!(binary[0], 255, "帯内ダストは消えるべき");
        assert_eq!(binary[(20 * w + 20) as usize], 0, "内側ストロークは残るべき");
        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 1);
        assert!(!q.needs_review, "帯内ダスト（speck降格）単独では要確認にしない");
    }

    #[test]
    fn gate_degenerate_4x4_zeroed_review_no_panic() {
        // 縮退サイズ（4x4 = 全画素が帯内）でもパニックせず、
        // 微小成分の除去でゼロ化した場合は needs_review が立つ。
        let w = 4u32;
        let h = 4u32;
        let mut binary = white_binary(w, h);
        fill_black(&mut binary, w, 1, 1, 3, 3); // 2x2 = 面積4（speck降格で除去）

        let q = apply_cell_quality_gate(&mut binary, w, h);

        assert!(binary.iter().all(|&v| v == 255), "縮退セルの微小成分は消えるべき");
        assert_eq!(q.removed_components, 1);
        assert_eq!(q.kept_components, 0);
        assert!(q.needs_review, "残成分ゼロ化は要確認");
    }

    #[test]
    fn gate_short_buffer_returns_empty_quality() {
        // binary.len() < w*h の不正入力は CellQuality::empty() を返し、バッファは無変更
        let mut short = vec![0u8; 5];
        let original = short.clone();
        let q = apply_cell_quality_gate(&mut short, 4, 4);
        assert_eq!(short, original, "不正長入力は無変更のはず");
        assert_eq!(q.removed_components, 0);
        assert_eq!(q.kept_components, 0);
        assert_eq!(q.ink_ratio, 0.0);
        assert!(!q.needs_review);
    }

    // ── measure_inner_black_ratio ──

    #[test]
    fn inner_ratio_all_white() {
        let img = make_uniform_image(100, 100, Rgba([255, 255, 255, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn inner_ratio_all_black() {
        // 均一黒画像: Sauvola単独ではコントラストがないため「背景」と誤判定していた
        // （#136 の空洞化バグそのもの: 太い筆ペン線の内部が窓全体インクで埋まるのと同じ状況）。
        // ハイブリッド二値化（#136）はグローバル閾値が輝度0を確実にインク判定するため、
        // 均一黒画像は黒比率1.0（全面インク）になるのが正しい挙動。
        let img = make_uniform_image(100, 100, Rgba([0, 0, 0, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert!((ratio - 1.0).abs() < 0.01, "ratio={ratio} should be ~1.0 (hybrid: global threshold catches uniform ink)");
    }

    #[test]
    fn inner_ratio_tiny_image_no_underflow() {
        // 1x1画像: margin_ratio=0.2 → margin=0, inner=1
        // margin_x(0) >= inner_w(1) は false なので計算される
        // ただし非常に小さい画像でもパニックしないことが重要
        let img = make_uniform_image(1, 1, Rgba([255, 255, 255, 255]));
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert!(ratio >= 0.0 && ratio <= 1.0);
    }

    #[test]
    fn inner_ratio_zero_size_image() {
        // 0x0画像 → saturating_sub で安全に0.0を返す
        let img = RgbaImage::new(0, 0);
        let ratio = measure_inner_black_ratio(&img, 0.2);
        assert_eq!(ratio, 0.0);
    }

    // ── Sauvola 二値化テスト ──

    #[test]
    fn sauvola_uniform_white() {
        // 均一白画像 → 全白（黒ピクセルなし）
        let w = 50u32;
        let h = 50u32;
        let gray = vec![255u8; (w * h) as usize];
        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
        let black_count = binary.iter().filter(|&&v| v == 0).count();
        assert_eq!(black_count, 0, "均一白画像は全白であるべき");
    }

    #[test]
    fn sauvola_uniform_black() {
        // 均一黒画像 → Sauvolaでは均一領域にコントラストがないため全白になる
        // （mean=0, threshold=0, val<threshold は偽）
        // これはSauvola法の正しい挙動: 均一領域は「背景」と判定される
        let w = 50u32;
        let h = 50u32;
        let gray = vec![0u8; (w * h) as usize];
        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);
        let black_count = binary.iter().filter(|&&v| v == 0).count();
        assert_eq!(black_count, 0, "均一黒画像はSauvolaでは全白（コントラストなし）");
    }

    #[test]
    fn sauvola_detects_text_on_both_halves() {
        // 左半分が暗い背景(100) + 暗い文字(30)、右半分が明るい背景(230) + 暗い文字(30)
        // Sauvolaの本領: 両方の「文字」部分を黒として検出する
        let w = 100u32;
        let h = 50u32;
        let mut gray = vec![0u8; (w * h) as usize];

        for y in 0..h {
            for x in 0..w {
                let idx = (y * w + x) as usize;
                if x < 50 {
                    // 左半分: 暗い背景
                    gray[idx] = 100;
                } else {
                    // 右半分: 明るい背景
                    gray[idx] = 230;
                }
            }
        }

        // 左半分に「文字」（暗いピクセル）を配置
        for y in 20..30 {
            for x in 20..30 {
                gray[(y * w + x) as usize] = 30;
            }
        }

        // 右半分に「文字」（暗いピクセル）を配置
        for y in 20..30 {
            for x in 70..80 {
                gray[(y * w + x) as usize] = 30;
            }
        }

        let binary = sauvola_binarize(&gray, w, h, SAUVOLA_K, SAUVOLA_WINDOW);

        // 左半分の文字領域に黒ピクセルがある
        let left_text_black = (20..30u32).flat_map(|y| (20..30u32).map(move |x| (y, x)))
            .filter(|&(y, x)| binary[(y * w + x) as usize] == 0)
            .count();

        // 右半分の文字領域に黒ピクセルがある
        let right_text_black = (20..30u32).flat_map(|y| (70..80u32).map(move |x| (y, x)))
            .filter(|&(y, x)| binary[(y * w + x) as usize] == 0)
            .count();

        assert!(left_text_black > 50, "左半分の文字を検出すべき（検出={left_text_black}/100）");
        assert!(right_text_black > 50, "右半分の文字を検出すべき（検出={right_text_black}/100）");
    }

    #[test]
    fn sauvola_empty_image() {
        // 0x0画像 → 空のVecを返す
        let binary = sauvola_binarize(&[], 0, 0, SAUVOLA_K, SAUVOLA_WINDOW);
        assert!(binary.is_empty());
    }

    #[test]
    fn rect_sum_single_pixel_at_origin() {
        // 3x3画像: 全ピクセル値1のIntegral Image
        // sum = [[1,2,3],[2,4,6],[3,6,9]]
        let integral: Vec<i64> = vec![1, 2, 3, 2, 4, 6, 3, 6, 9];
        // (0,0)〜(0,0)の1ピクセル = 1
        assert_eq!(rect_sum(&integral, 3, 0, 0, 0, 0), 1);
        // (0,0)〜(2,2)の全体 = 9
        assert_eq!(rect_sum(&integral, 3, 0, 0, 2, 2), 9);
        // (1,1)〜(2,2)の右下4ピクセル = 4
        assert_eq!(rect_sum(&integral, 3, 1, 1, 2, 2), 4);
    }

    // ── モルフォロジカル処理テスト ──

    #[test]
    fn erode_removes_isolated_black_pixel() {
        // 白背景に孤立1pxの黒点 → Erodeで消える
        let w = 5u32;
        let h = 5u32;
        let mut binary = vec![255u8; (w * h) as usize];
        binary[(2 * w + 2) as usize] = 0; // 中央に黒1px
        let result = morphological_erode(&binary, w, h);
        // 孤立黒点は周囲が白なので白に変わる
        assert_eq!(result[(2 * w + 2) as usize], 255, "孤立黒点がErodeで消えるべき");
        // 全体が白であること
        assert!(result.iter().all(|&v| v == 255), "全ピクセルが白であるべき");
    }

    #[test]
    fn dilate_removes_isolated_white_pixel() {
        // 黒背景に孤立1pxの白点 → Dilateで消える
        let w = 5u32;
        let h = 5u32;
        let mut binary = vec![0u8; (w * h) as usize];
        binary[(2 * w + 2) as usize] = 255; // 中央に白1px
        let result = morphological_dilate(&binary, w, h);
        // 孤立白点は周囲が黒なので黒に変わる
        assert_eq!(result[(2 * w + 2) as usize], 0, "孤立白点がDilateで消えるべき");
        // 全体が黒であること
        assert!(result.iter().all(|&v| v == 0), "全ピクセルが黒であるべき");
    }

    #[test]
    fn open_close_removes_noise_preserves_stroke() {
        // 白背景に太いストローク（3×3黒ブロック）+ 孤立黒ノイズ1px
        let w = 10u32;
        let h = 10u32;
        let mut binary = vec![255u8; (w * h) as usize];

        // 3×3の黒ブロック（太いストローク）を (3,3)-(5,5) に配置
        for y in 3..6u32 {
            for x in 3..6u32 {
                binary[(y * w + x) as usize] = 0;
            }
        }
        // 孤立黒ノイズ1px を (0,0) に配置
        binary[0] = 0;

        let result = morphological_open_close(&binary, w, h);

        // 孤立ノイズは消えるべき
        assert_eq!(result[0], 255, "孤立黒ノイズがOpeningで消えるべき");

        // 太いストロークの中心は保持されるべき
        assert_eq!(result[(4 * w + 4) as usize], 0, "ストローク中心は保持されるべき");
    }

    #[test]
    fn morphological_empty_image() {
        // 0x0画像 → 空のVecを返す
        assert!(morphological_erode(&[], 0, 0).is_empty());
        assert!(morphological_dilate(&[], 0, 0).is_empty());
        assert!(morphological_open_close(&[], 0, 0).is_empty());
    }

    // ── CLAHE テスト ──

    #[test]
    fn clahe_uniform_image() {
        // 均一画像（全ピクセル128）→ 出力も均一
        let w = 64u32;
        let h = 64u32;
        let gray = vec![128u8; (w * h) as usize];
        let result = apply_clahe(&gray, w, h);
        assert_eq!(result.len(), (w * h) as usize);
        let first = result[0];
        assert!(result.iter().all(|&v| v == first), "均一画像のCLAHE出力は均一であるべき");
    }

    #[test]
    fn clahe_preserves_dimensions() {
        // 入出力のサイズが同じ
        let w = 80u32;
        let h = 60u32;
        let gray: Vec<u8> = (0..(w * h)).map(|i| (i % 256) as u8).collect();
        let result = apply_clahe(&gray, w, h);
        assert_eq!(result.len(), gray.len(), "CLAHE出力サイズが入力と一致すべき");
    }

    #[test]
    fn clahe_improves_contrast() {
        // 低コントラスト画像（128±10）→ ダイナミックレンジが広がる
        let w = 64u32;
        let h = 64u32;
        let gray: Vec<u8> = (0..(w * h))
            .map(|i| (118 + (i % 21)) as u8) // 118..=138
            .collect();
        let input_min = *gray.iter().min().unwrap();
        let input_max = *gray.iter().max().unwrap();
        let input_range = input_max - input_min; // 20

        let result = apply_clahe(&gray, w, h);
        let output_min = *result.iter().min().unwrap();
        let output_max = *result.iter().max().unwrap();
        let output_range = output_max - output_min;

        assert!(
            output_range > input_range,
            "CLAHEでダイナミックレンジが広がるべき: input={input_range}, output={output_range}"
        );
    }

    #[test]
    fn clahe_tiny_image() {
        // タイルサイズが0になるような極小画像でパニックしないこと
        let gray = vec![128u8; 9]; // 3x3
        let result = apply_clahe(&gray, 3, 3);
        assert_eq!(result.len(), 9);

        // 0x0
        let result = apply_clahe(&[], 0, 0);
        assert!(result.is_empty());

        // 1x1
        let result = apply_clahe(&[200], 1, 1);
        assert_eq!(result.len(), 1);
    }

    // ── モアレ検出・除去テスト ──

    #[test]
    fn detect_moire_uniform() {
        // 均一画像はモアレなし
        let w = 64u32;
        let h = 64u32;
        let gray = vec![128u8; (w * h) as usize];
        assert!(!detect_moire(&gray, w, h), "均一画像はモアレなしであるべき");
    }

    #[test]
    fn detect_moire_stripe_pattern() {
        // 縞模様画像はモアレあり判定
        let w = 64u32;
        let h = 64u32;
        let gray: Vec<u8> = (0..(w * h))
            .map(|i| {
                let x = i % w;
                if x % 2 == 0 { 0 } else { 255 }
            })
            .collect();
        assert!(detect_moire(&gray, w, h), "縞模様画像はモアレありであるべき");
    }

    #[test]
    fn median_filter_removes_salt_pepper() {
        // ソルト&ペッパーノイズをメディアンフィルタが除去
        let w = 8u32;
        let h = 8u32;
        let mut gray = vec![128u8; (w * h) as usize];
        // ノイズを注入（内側のピクセル）
        gray[(3 * w + 3) as usize] = 255; // salt
        gray[(4 * w + 4) as usize] = 0;   // pepper

        let result = median_filter_3x3(&gray, w, h);
        assert_eq!(result.len(), (w * h) as usize);
        // ノイズが除去され、周囲と同じ値（128）になるべき
        assert_eq!(result[(3 * w + 3) as usize], 128, "ソルトノイズが除去されるべき");
        assert_eq!(result[(4 * w + 4) as usize], 128, "ペッパーノイズが除去されるべき");
    }

    #[test]
    fn median_filter_empty() {
        // 空画像で空Vec
        let result = median_filter_3x3(&[], 0, 0);
        assert!(result.is_empty(), "空画像は空Vecを返すべき");
    }
}
