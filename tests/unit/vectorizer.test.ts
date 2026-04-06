/**
 * ベクター化モジュールのユニットテスト
 * 合成ImageDataに黒い矩形を描いて、輪郭抽出→ベジェ変換を検証
 */
import { describe, it, expect } from 'vitest';
import { vectorizeGlyph } from '../../src/lib/vectorizer/contour';

describe('Vectorizer', () => {
  it('should extract contours from a simple black square', () => {
    // ImageData を直接構築: 背景=200, 文字=30, エッジ=100
    // node-canvas の filter 非対応を回避しつつ大津の閾値が中間に落ちるようにする
    const size = 128;
    const data = new Uint8ClampedArray(size * size * 4);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const idx = (y * size + x) * 4;
        let gray = 200;
        const inSquare = x >= 30 && x < 98 && y >= 30 && y < 98;
        const onEdge = inSquare && (x === 30 || x === 97 || y === 30 || y === 97);
        if (inSquare && !onEdge) gray = 30;
        else if (onEdge) gray = 100;
        data[idx] = gray;
        data[idx + 1] = gray;
        data[idx + 2] = gray;
        data[idx + 3] = 255;
      }
    }
    const imageData = { data, width: size, height: size };
    const paths = vectorizeGlyph(imageData as any);

    expect(paths.length).toBeGreaterThan(0);

    // 各パスが M で始まり Z で終わること
    for (const p of paths) {
      expect(p[0].type).toBe('M');
      expect(p[p.length - 1].type).toBe('Z');
    }
  });

  it('should return empty for a blank white image', () => {
    // 均一グレー（大津では全ピクセル同一値→黒なし）
    const size = 128;
    const data = new Uint8ClampedArray(size * size * 4);
    for (let i = 0; i < size * size; i++) {
      data[i * 4] = 200;
      data[i * 4 + 1] = 200;
      data[i * 4 + 2] = 200;
      data[i * 4 + 3] = 255;
    }
    const imageData = { data, width: size, height: size };
    const paths = vectorizeGlyph(imageData as any);

    expect(paths.length).toBe(0);
  });

  it('should produce paths with coordinates in font units (0-1000)', () => {
    // 円をImageDataで直接構築
    const size = 128;
    const data = new Uint8ClampedArray(size * size * 4);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const idx = (y * size + x) * 4;
        const dist = Math.sqrt((x - 64) ** 2 + (y - 64) ** 2);
        let gray = 200;
        if (dist < 28) gray = 30;
        else if (dist < 32) gray = 100; // エッジのアンチエイリアス模倣
        data[idx] = gray;
        data[idx + 1] = gray;
        data[idx + 2] = gray;
        data[idx + 3] = 255;
      }
    }
    const imageData = { data, width: size, height: size };
    const paths = vectorizeGlyph(imageData as any);

    expect(paths.length).toBeGreaterThan(0);

    // 全座標がフォント座標系の範囲内であること
    for (const p of paths) {
      for (const cmd of p) {
        expect(cmd.x).toBeGreaterThanOrEqual(-100);
        expect(cmd.x).toBeLessThanOrEqual(1100);
        expect(cmd.y).toBeGreaterThanOrEqual(-100);
        expect(cmd.y).toBeLessThanOrEqual(1100);
      }
    }
  });
});
