import { describe, it, expect } from 'vitest';
import { translateWasmError } from '../../src/lib/scanner/processor';

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
});
