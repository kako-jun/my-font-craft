/**
 * tests/fixtures/distort.ts（擬似歪み生成、#109）のユニットテスト。
 *
 * - computeHomography / applyHomography: 解析解との厳密比較・異常系
 * - computeDistortedCorners: 回転・台形の座標計算
 * - distortPng: 余白・背景・バイリニア境界・明度ムラ・ぼかしの実画素検証
 */

import { describe, it, expect } from 'vitest';
import { createCanvas, loadImage } from 'canvas';
import {
  computeHomography,
  applyHomography,
  computeDistortedCorners,
  distortPng,
  type Point,
} from '../fixtures/distort';

// --- テスト用ヘルパー ---

/** (x,y) ごとに RGB を指定して PNG バッファを作る */
function makePng(
  w: number,
  h: number,
  fill: (x: number, y: number) => [number, number, number],
): Buffer {
  const canvas = createCanvas(w, h);
  const ctx = canvas.getContext('2d');
  const img = ctx.createImageData(w, h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const [r, g, b] = fill(x, y);
      const i = (y * w + x) * 4;
      img.data[i] = r;
      img.data[i + 1] = g;
      img.data[i + 2] = b;
      img.data[i + 3] = 255;
    }
  }
  ctx.putImageData(img, 0, 0);
  return canvas.toBuffer('image/png');
}

interface Pixels {
  w: number;
  h: number;
  data: Uint8ClampedArray;
}

/** PNG バッファをデコードして RGBA 配列を返す */
async function readPixels(buf: Buffer): Promise<Pixels> {
  const img = await loadImage(buf);
  const canvas = createCanvas(img.width, img.height);
  const ctx = canvas.getContext('2d');
  ctx.drawImage(img, 0, 0);
  return {
    w: img.width,
    h: img.height,
    data: ctx.getImageData(0, 0, img.width, img.height).data,
  };
}

/** (x,y) の RGB を取り出す */
function rgbAt(p: Pixels, x: number, y: number): [number, number, number] {
  const i = (y * p.w + x) * 4;
  return [p.data[i], p.data[i + 1], p.data[i + 2]];
}

describe('computeHomography / applyHomography', () => {
  it('恒等対応（src = dst）では単位行列を返す', () => {
    // 正方形でない一般の4点で恒等対応を解く
    const corners: Point[] = [
      { x: 10, y: 20 },
      { x: 110, y: 25 },
      { x: 15, y: 120 },
      { x: 105, y: 115 },
    ];
    const h = computeHomography(corners, corners);
    const identity = [1, 0, 0, 0, 1, 0, 0, 0, 1];
    for (let i = 0; i < 9; i++) {
      expect(Math.abs(h[i] - identity[i])).toBeLessThan(1e-9);
    }
  });

  it('単位正方形→2倍+平行移動の対応で内部点が解析解どおりに写る', () => {
    // (x,y) → (2x+3, 2y+5)。ホモグラフィは h = [2,0,3, 0,2,5, 0,0,1]
    const src: Point[] = [
      { x: 0, y: 0 },
      { x: 1, y: 0 },
      { x: 0, y: 1 },
      { x: 1, y: 1 },
    ];
    const dst: Point[] = [
      { x: 3, y: 5 },
      { x: 5, y: 5 },
      { x: 3, y: 7 },
      { x: 5, y: 7 },
    ];
    const h = computeHomography(src, dst);
    const expected = [2, 0, 3, 0, 2, 5, 0, 0, 1];
    for (let i = 0; i < 9; i++) {
      expect(Math.abs(h[i] - expected[i])).toBeLessThan(1e-12);
    }
    // 対応点として与えていない内部点・外部点の写像先も解析解と一致する
    const center = applyHomography(h, 0.5, 0.5);
    expect(Math.abs(center.x - 4)).toBeLessThan(1e-12);
    expect(Math.abs(center.y - 6)).toBeLessThan(1e-12);
    const outer = applyHomography(h, 2, 3);
    expect(Math.abs(outer.x - 7)).toBeLessThan(1e-12);
    expect(Math.abs(outer.y - 11)).toBeLessThan(1e-12);
  });

  it('順方向と逆方向を独立に解いた往復写像が元の点に戻る（射影成分あり）', () => {
    // 台形対応（h31/h32 が非ゼロになる真の射影変換）
    const src: Point[] = [
      { x: 0, y: 0 },
      { x: 100, y: 0 },
      { x: 0, y: 100 },
      { x: 100, y: 100 },
    ];
    const dst: Point[] = [
      { x: 15, y: 5 },
      { x: 85, y: 8 },
      { x: 2, y: 95 },
      { x: 98, y: 92 },
    ];
    const h = computeHomography(src, dst);
    const hInv = computeHomography(dst, src);
    const probes: Point[] = [
      { x: 50, y: 50 },
      { x: 10, y: 90 },
      { x: 99, y: 1 },
      { x: 33.3, y: 66.7 },
    ];
    for (const p of probes) {
      const there = applyHomography(h, p.x, p.y);
      const back = applyHomography(hInv, there.x, there.y);
      expect(Math.abs(back.x - p.x)).toBeLessThan(1e-6);
      expect(Math.abs(back.y - p.y)).toBeLessThan(1e-6);
    }
  });

  it('対応点が4点でないと throw する', () => {
    const three: Point[] = [
      { x: 0, y: 0 },
      { x: 1, y: 0 },
      { x: 0, y: 1 },
    ];
    const four: Point[] = [...three, { x: 1, y: 1 }];
    const five: Point[] = [...four, { x: 2, y: 2 }];
    expect(() => computeHomography(three, four)).toThrow('4点対応が必要です');
    expect(() => computeHomography(four, three)).toThrow('4点対応が必要です');
    expect(() => computeHomography(five, five)).toThrow('4点対応が必要です');
  });

  it('退化した対応点（同一点・一直線）では特異行列として throw する', () => {
    const square: Point[] = [
      { x: 0, y: 0 },
      { x: 1, y: 0 },
      { x: 0, y: 1 },
      { x: 1, y: 1 },
    ];
    // 3点が同一
    const collapsed: Point[] = [
      { x: 5, y: 5 },
      { x: 5, y: 5 },
      { x: 5, y: 5 },
      { x: 1, y: 1 },
    ];
    expect(() => computeHomography(collapsed, square)).toThrow('特異行列');
    // 全点が一直線
    const collinear: Point[] = [
      { x: 0, y: 0 },
      { x: 1, y: 1 },
      { x: 2, y: 2 },
      { x: 3, y: 3 },
    ];
    expect(() => computeHomography(collinear, square)).toThrow('特異行列');
  });
});

describe('computeDistortedCorners', () => {
  it('デフォルト（無回転・無台形・等倍）では出力中心の軸平行矩形になる', () => {
    // src 100x80, out 140x120 → 中心 (70,60)、半幅 50、半高 40
    const corners = computeDistortedCorners(100, 80, 140, 120, {});
    expect(corners).toEqual([
      { x: 20, y: 20 }, // TL
      { x: 120, y: 20 }, // TR
      { x: 20, y: 100 }, // BL
      { x: 120, y: 100 }, // BR
    ]);
  });

  it('rotateDeg=90 で4隅が解析解（TL→右上へ90度回転）と一致する', () => {
    // halfW=50, halfH=40, 中心 (70,60)。時計回り90度: (x,y) → (-y, x)
    // TL(-50,-40) → (40,-50), TR(50,-40) → (40,50), BL(-50,40) → (-40,-50), BR(50,40) → (-40,50)
    const corners = computeDistortedCorners(100, 80, 140, 120, { rotateDeg: 90 });
    const expected: Point[] = [
      { x: 110, y: 10 }, // TL
      { x: 110, y: 110 }, // TR
      { x: 30, y: 10 }, // BL
      { x: 30, y: 110 }, // BR
    ];
    for (let i = 0; i < 4; i++) {
      // cos(90°) は浮動小数点上ゼロにならない（≈6e-17）ため閾値比較
      expect(Math.abs(corners[i].x - expected[i].x)).toBeLessThan(1e-10);
      expect(Math.abs(corners[i].y - expected[i].y)).toBeLessThan(1e-10);
    }
  });

  it('trapezoid の符号と強度で上辺の狭まり方が変わる（0 / 0.05 / -0.05）', () => {
    const base = computeDistortedCorners(100, 80, 140, 120, { trapezoid: 0 });
    // t=0 は無変形（デフォルト矩形と同一）
    expect(base).toEqual([
      { x: 20, y: 20 },
      { x: 120, y: 20 },
      { x: 20, y: 100 },
      { x: 120, y: 100 },
    ]);

    // t=0.05: 上辺が狭くなる（TL.x は +halfW*t、TR.x は -halfW*t）
    const t = computeDistortedCorners(100, 80, 140, 120, { trapezoid: 0.05 });
    expect(t[0].x).toBeCloseTo(20 + 50 * 0.05, 12); // TL: 22.5
    expect(t[1].x).toBeCloseTo(120 - 50 * 0.05, 12); // TR: 117.5
    expect(t[0].y).toBeCloseTo(20 - 40 * 0.05 * 0.3, 12); // TL.y: 19.4
    expect(t[2].x).toBeCloseTo(20 - 50 * 0.05 * 0.5, 12); // BL: 18.75
    expect(t[2].y).toBeCloseTo(100 + 40 * 0.05 * 0.2, 12); // BL.y: 100.4

    // t=-0.05: 逆台形（上辺が広くなる）
    const n = computeDistortedCorners(100, 80, 140, 120, { trapezoid: -0.05 });
    expect(n[0].x).toBeCloseTo(20 - 50 * 0.05, 12); // TL: 17.5
    expect(n[1].x).toBeCloseTo(120 + 50 * 0.05, 12); // TR: 122.5
  });
});

describe('distortPng', () => {
  it('歪み無し + padding:0 では出力サイズ・全ピクセルが入力と一致する', async () => {
    // バイリニア再標本化は (W-1)/W のスケールを含むため、非一様画像では
    // 補間により厳密一致しない（実装仕様）。一様色なら補間結果も同色になるので
    // 「背景の混入なし・明度変化なし・値の劣化なし」を厳密に検証できる。
    const input = makePng(4, 4, () => [90, 120, 150]);
    const out = await readPixels(await distortPng(input, { padding: 0 }));
    expect(out.w).toBe(4);
    expect(out.h).toBe(4);
    for (let y = 0; y < out.h; y++) {
      for (let x = 0; x < out.w; x++) {
        expect(rgbAt(out, x, y), `(${x},${y})`).toEqual([90, 120, 150]);
      }
    }
  });

  it('バイリニア境界: 画像端 sx=srcW-1 は「内側」扱いになる（<= と < の差を検出）', async () => {
    // 白 4x4 + padding:1 + 青背景 → 出力 6x6。
    // 出力 (dx,dy) は src ((dx-1)*3/4, (dy-1)*3/4) に写る:
    // - dx=0 / dy=0 → 負 → 背景
    // - dx=5 / dy=5 → ちょうど srcW-1=3 → 判定が `<=` なので画像内（白）。
    //   実装が `<` に変わるとこの行・列が背景になり、このテストが落ちる。
    // - dx=1 / dy=1 は写像先がちょうど 0 で、ガウス消去の丸めにより ±ε に
    //   振れうるナイフエッジのため、意図的に検証対象から外す。
    const input = makePng(4, 4, () => [255, 255, 255]);
    const out = await readPixels(await distortPng(input, { padding: 1, background: [0, 0, 255] }));
    expect(out.w).toBe(6);
    expect(out.h).toBe(6);
    const BG: [number, number, number] = [0, 0, 255];
    const WHITE: [number, number, number] = [255, 255, 255];
    // 上端行・左端列（写像先が確実に負）は背景
    for (let i = 0; i < 6; i++) {
      expect(rgbAt(out, i, 0), `top (${i},0)`).toEqual(BG);
      expect(rgbAt(out, 0, i), `left (0,${i})`).toEqual(BG);
    }
    // 右端列 dx=5（sx=3=srcW-1）・下端行 dy=5（sy=3=srcH-1）は画像内=白
    for (let i = 2; i <= 5; i++) {
      expect(rgbAt(out, 5, i), `right (5,${i})`).toEqual(WHITE);
      expect(rgbAt(out, i, 5), `bottom (${i},5)`).toEqual(WHITE);
    }
    // 内部は白
    for (let y = 2; y <= 4; y++) {
      for (let x = 2; x <= 4; x++) {
        expect(rgbAt(out, x, y), `inner (${x},${y})`).toEqual(WHITE);
      }
    }
  });

  it('padding:200 では出力が入力+400になり四隅は背景色で埋まる', async () => {
    const bg: [number, number, number] = [10, 20, 30];
    const input = makePng(8, 8, () => [255, 0, 0]);
    const out = await readPixels(await distortPng(input, { padding: 200, background: bg }));
    expect(out.w).toBe(8 + 400);
    expect(out.h).toBe(8 + 400);
    expect(rgbAt(out, 0, 0)).toEqual(bg);
    expect(rgbAt(out, out.w - 1, 0)).toEqual(bg);
    expect(rgbAt(out, 0, out.h - 1)).toEqual(bg);
    expect(rgbAt(out, out.w - 1, out.h - 1)).toEqual(bg);
  });

  it('brightnessGradient で上端が明るく下端が暗くなり、白はクランプで255のまま', async () => {
    // 一様グレー128: 上端 128*1.05=134、下端 128*0.95≈122
    const gray = makePng(4, 8, () => [128, 128, 128]);
    const outGray = await readPixels(
      await distortPng(gray, { padding: 0, brightnessGradient: 0.1 }),
    );
    const [topR] = rgbAt(outGray, 0, 0);
    const [bottomR] = rgbAt(outGray, 0, outGray.h - 1);
    expect(topR).toBe(134);
    expect(bottomR).toBe(122);
    expect(topR).toBeGreaterThan(bottomR);

    // 純白 255 は上端の増幅（×1.05）でもクランプされ 255 のまま
    const white = makePng(4, 8, () => [255, 255, 255]);
    const outWhite = await readPixels(
      await distortPng(white, { padding: 0, brightnessGradient: 0.1 }),
    );
    expect(rgbAt(outWhite, 0, 0)).toEqual([255, 255, 255]);
  });

  it('blur は内部を3x3平均化し、最外周1行1列には触れない', async () => {
    // 白地の (0,0) に黒1px。blur 有無で同条件の出力を比較する:
    // - 最外周（y=0, y=h-1, x=0, x=w-1）は blur 前後で完全一致
    //   （ループが 1..h-2 / 1..w-2 なので端は書き換えない。0..h-1 に変わると落ちる）
    // - 内部の各ピクセルは「blur 無し出力の3x3平均（round）」と一致
    const input = makePng(6, 6, (x, y) => (x === 0 && y === 0 ? [0, 0, 0] : [255, 255, 255]));
    const plain = await readPixels(await distortPng(input, { padding: 0 }));
    const blurred = await readPixels(await distortPng(input, { padding: 0, blur: true }));
    expect(blurred.w).toBe(plain.w);
    expect(blurred.h).toBe(plain.h);

    // 最外周は不変
    for (let x = 0; x < plain.w; x++) {
      expect(rgbAt(blurred, x, 0), `top (${x},0)`).toEqual(rgbAt(plain, x, 0));
      expect(rgbAt(blurred, x, plain.h - 1), `bottom (${x},${plain.h - 1})`).toEqual(
        rgbAt(plain, x, plain.h - 1),
      );
    }
    for (let y = 0; y < plain.h; y++) {
      expect(rgbAt(blurred, 0, y), `left (0,${y})`).toEqual(rgbAt(plain, 0, y));
      expect(rgbAt(blurred, plain.w - 1, y), `right (${plain.w - 1},${y})`).toEqual(
        rgbAt(plain, plain.w - 1, y),
      );
    }

    // 内部 = 3x3 平均（blur 無し出力から期待値を計算）
    for (let y = 1; y < plain.h - 1; y++) {
      for (let x = 1; x < plain.w - 1; x++) {
        for (let c = 0; c < 3; c++) {
          let sum = 0;
          for (let ky = -1; ky <= 1; ky++) {
            for (let kx = -1; kx <= 1; kx++) {
              sum += plain.data[((y + ky) * plain.w + (x + kx)) * 4 + c];
            }
          }
          expect(blurred.data[(y * plain.w + x) * 4 + c], `(${x},${y}) ch${c}`).toBe(
            Math.round(sum / 9),
          );
        }
      }
    }

    // 黒1px の影響が実際に及ぶ (1,1) は blur で変化している（テストが空振りしていない証明）
    expect(rgbAt(blurred, 1, 1)).not.toEqual(rgbAt(plain, 1, 1));
  });

  it('オプション空 {} でも throw せずデフォルト（padding:200・机色背景）で動く', async () => {
    const input = makePng(4, 4, () => [0, 0, 0]);
    const out = await readPixels(await distortPng(input, {}));
    expect(out.w).toBe(4 + 400);
    expect(out.h).toBe(4 + 400);
    // デフォルト背景 [200,195,185]
    expect(rgbAt(out, 0, 0)).toEqual([200, 195, 185]);
  });
});
