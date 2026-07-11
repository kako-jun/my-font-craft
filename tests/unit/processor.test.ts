import { describe, it, expect } from 'vitest';
import { inferFailedStage, translateWasmError } from '../../src/lib/scanner/processor';

describe('translateWasmError', () => {
  it('マーカー検出失敗を撮影ガイド付きメッセージに変換する', () => {
    const raw = 'TopLeft マーカーが検出できませんでした（ブロブ数=0, フィルタ通過=0）';
    const result = translateWasmError(raw);
    expect(result).toContain('正面から撮影');
    expect(result).not.toContain('ブロブ');
  });

  it('BR マーカーのエラーも同様に変換する', () => {
    const raw = 'BottomRight マーカーが検出できませんでした（ブロブ数=3, フィルタ通過=0）';
    const result = translateWasmError(raw);
    expect(result).toContain('正面から撮影');
  });

  it('DPI不足エラーはRustのメッセージをそのまま返す', () => {
    const raw =
      '解像度が低すぎます (147 DPI)。もう少し近づけて撮影してください（推奨: 300DPI以上）';
    const result = translateWasmError(raw);
    expect(result).toBe(raw);
  });

  it('未知のエラーはそのまま返す', () => {
    const raw = '画像デコードエラー: unsupported format';
    const result = translateWasmError(raw);
    expect(result).toBe(raw);
  });

  it('buildSha が与えられた場合は末尾に [build: sha] を付加する', () => {
    const raw = '画像デコードエラー: unsupported format';
    const result = translateWasmError(raw, 'abc1234');
    expect(result).toBe(`${raw} [build: abc1234]`);
  });

  it('buildSha が null の場合は build タグを付加しない', () => {
    const raw = '画像デコードエラー: unsupported format';
    const result = translateWasmError(raw, null);
    expect(result).toBe(raw);
  });
});

describe('inferFailedStage', () => {
  // 代表文言は Rust 側の実メッセージ（cli/src/marker.rs / cli/src/pipeline.rs）をコピーしたもの。
  // Rust 側の文言が変わったらここも追従する。

  it('マーカー検出失敗の実文言を marker と判定する', () => {
    // cli/src/marker.rs
    const raw = 'TopLeft マーカーが検出できませんでした（ブロブ数=0, フィルタ通過=0）';
    expect(inferFailedStage(raw)).toBe('marker');
  });

  it('解像度不足の実文言を dpi と判定する', () => {
    // cli/src/pipeline.rs
    const raw =
      '解像度が低すぎます (147 DPI)。もう少し近づけて撮影してください（推奨: 300DPI以上）';
    expect(inferFailedStage(raw)).toBe('dpi');
  });

  it('レンズ歪み過大の実文言を perspective と判定する', () => {
    // cli/src/pipeline.rs（歪み + 撮り直し の両キーワードを含む）
    const raw =
      'レンズ歪みが大きすぎます（中心残差 3.5mm、許容 2.0mm以下）。もう一歩離れて撮り直してください。';
    expect(inferFailedStage(raw)).toBe('perspective');
  });

  it('画像デコード/フォーマット失敗の実文言を decode と判定する', () => {
    // cli/src/pipeline.rs
    expect(inferFailedStage('画像デコードエラー: unsupported format')).toBe('decode');
    expect(inferFailedStage('画像フォーマット推定エラー: unknown magic')).toBe('decode');
  });

  it('どの段階キーワードにも該当しない実文言は wasm にフォールバックする', () => {
    // cli/src/pipeline.rs。「デコーダ」は「デコード」と部分一致しないため decode にならない
    expect(inferFailedStage('デコーダ初期化エラー: out of memory')).toBe('wasm');
    expect(inferFailedStage('unreachable executed')).toBe('wasm');
  });

  it('複数キーワード同時ヒット時は marker > dpi > perspective > decode の先勝ちが崩れない', () => {
    // 注意（将来リスク）: 「マーカー」のマッチは意図的に最優先だが判定が広い。
    // 将来 Rust 側の perspective/dpi 系メッセージが「マーカー」という語を含むように
    // 変わると marker に誤分類される。文言変更時はこのテストで優先順位を再確認すること。
    expect(inferFailedStage('歪み補正中にマーカー座標が不正でした')).toBe('marker');
    expect(inferFailedStage('解像度不足でマーカーが検出できませんでした')).toBe('marker');
    expect(inferFailedStage('解像度が低く歪み推定に失敗しました')).toBe('dpi');
    expect(inferFailedStage('歪みが大きく画像のデコード結果を補正できません')).toBe('perspective');
  });

  it('decode 判定は「画像」と「デコード/フォーマット」の AND（片翼のみでは wasm）', () => {
    expect(inferFailedStage('画像読み込みエラー: file not found')).toBe('wasm'); // 画像 のみ
    expect(inferFailedStage('デコードに失敗しました')).toBe('wasm'); // デコード のみ
  });

  it('空文字は wasm にフォールバックする', () => {
    expect(inferFailedStage('')).toBe('wasm');
  });
});
