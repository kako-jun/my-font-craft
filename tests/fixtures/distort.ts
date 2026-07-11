/**
 * 合成スキャン画像の擬似歪み生成（#109）
 *
 * 正面の模擬スキャン画像に「斜め撮影風」の歪みを加える。
 * cli/src/distort.rs と同系統のパラメータ（回転 + 台形 + グレー背景 + 余白）を
 * TypeScript / node-canvas 上で再現する。
 *
 * node-canvas の 2D コンテキストは射影変換をサポートしないため、
 * 4隅対応点からホモグラフィ（8自由度）を解き、
 * 出力→入力の逆写像 + バイリニア補間で自前実装する。
 */

import { createCanvas, loadImage } from 'canvas';

export interface Point {
  x: number;
  y: number;
}

export interface DistortOptions {
  /** 回転角度（度）。正で時計回り */
  rotateDeg?: number;
  /** 台形歪みの強さ（0.0=なし, 0.05=cli/src/distort.rs のデフォルト） */
  trapezoid?: number;
  /** 全体の縮小率（1.0=等倍） */
  scale?: number;
  /** 余白（px）。グレー背景で埋める */
  padding?: number;
  /** 背景色（机/床のシミュレート） */
  background?: [number, number, number];
  /** 明度ムラ: 上端→下端の輝度倍率の振れ幅（例 0.12 → 上 1.06倍〜下 0.94倍） */
  brightnessGradient?: number;
  /** 3x3 ボックスぼかしを適用するか（軽いピンボケのシミュレート） */
  blur?: boolean;
}

/**
 * 4点対応 (src[i] → dst[i]) からホモグラフィ行列（3x3、h33=1 固定）を解く。
 * 戻り値は行優先 [h11..h33] の 9 要素。
 */
export function computeHomography(src: Point[], dst: Point[]): number[] {
  if (src.length !== 4 || dst.length !== 4) {
    throw new Error('computeHomography: 4点対応が必要です');
  }
  // 8x8 線形系 A·h = b（h = [h11,h12,h13,h21,h22,h23,h31,h32]）
  const A: number[][] = [];
  const b: number[] = [];
  for (let i = 0; i < 4; i++) {
    const { x, y } = src[i];
    const { x: u, y: v } = dst[i];
    A.push([x, y, 1, 0, 0, 0, -u * x, -u * y]);
    b.push(u);
    A.push([0, 0, 0, x, y, 1, -v * x, -v * y]);
    b.push(v);
  }
  const h = solveLinearSystem(A, b);
  return [...h, 1];
}

/** ガウスの消去法（部分ピボット選択）で n×n 線形系を解く */
function solveLinearSystem(A: number[][], b: number[]): number[] {
  const n = b.length;
  // 拡大係数行列
  const m = A.map((row, i) => [...row, b[i]]);
  for (let col = 0; col < n; col++) {
    // ピボット選択
    let pivot = col;
    for (let row = col + 1; row < n; row++) {
      if (Math.abs(m[row][col]) > Math.abs(m[pivot][col])) pivot = row;
    }
    if (Math.abs(m[pivot][col]) < 1e-12) {
      throw new Error('solveLinearSystem: 特異行列（対応点が退化しています）');
    }
    [m[col], m[pivot]] = [m[pivot], m[col]];
    // 前進消去
    for (let row = col + 1; row < n; row++) {
      const f = m[row][col] / m[col][col];
      for (let k = col; k <= n; k++) m[row][k] -= f * m[col][k];
    }
  }
  // 後退代入
  const x = new Array<number>(n).fill(0);
  for (let row = n - 1; row >= 0; row--) {
    let sum = m[row][n];
    for (let k = row + 1; k < n; k++) sum -= m[row][k] * x[k];
    x[row] = sum / m[row][row];
  }
  return x;
}

/** ホモグラフィ行列を点に適用する */
export function applyHomography(h: number[], x: number, y: number): Point {
  const w = h[6] * x + h[7] * y + h[8];
  return {
    x: (h[0] * x + h[1] * y + h[2]) / w,
    y: (h[3] * x + h[4] * y + h[5]) / w,
  };
}

/**
 * 出力キャンバス上での元画像4隅の配置座標を計算する。
 * cli/src/distort.rs と同じ手順: 中心基準 → 縮小 → 回転 → 台形 → 中心オフセット。
 * 順序は [TL, TR, BL, BR]。
 */
export function computeDistortedCorners(
  srcW: number,
  srcH: number,
  outW: number,
  outH: number,
  opts: DistortOptions,
): Point[] {
  const rotateDeg = opts.rotateDeg ?? 0;
  const t = opts.trapezoid ?? 0;
  const scale = opts.scale ?? 1;

  const halfW = (srcW / 2) * scale;
  const halfH = (srcH / 2) * scale;
  const cx = outW / 2;
  const cy = outH / 2;

  const rad = (rotateDeg * Math.PI) / 180;
  const cosR = Math.cos(rad);
  const sinR = Math.sin(rad);

  const local: Point[] = [
    { x: -halfW, y: -halfH }, // TL
    { x: halfW, y: -halfH }, // TR
    { x: -halfW, y: halfH }, // BL
    { x: halfW, y: halfH }, // BR
  ];

  const rotated = local.map(({ x, y }) => ({
    x: x * cosR - y * sinR,
    y: x * sinR + y * cosR,
  }));

  // 台形変形: 上辺を狭く、下辺を広く（distort.rs と同じ係数）
  const trapezoided: Point[] = [
    { x: rotated[0].x + halfW * t, y: rotated[0].y - halfH * t * 0.3 },
    { x: rotated[1].x - halfW * t, y: rotated[1].y - halfH * t * 0.3 },
    { x: rotated[2].x - halfW * t * 0.5, y: rotated[2].y + halfH * t * 0.2 },
    { x: rotated[3].x + halfW * t * 0.5, y: rotated[3].y + halfH * t * 0.2 },
  ];

  return trapezoided.map(({ x, y }) => ({ x: cx + x, y: cy + y }));
}

/**
 * PNG バッファに擬似歪みを適用して PNG バッファを返す。
 * 出力サイズは 入力サイズ + 余白×2。
 */
export async function distortPng(input: Buffer, opts: DistortOptions): Promise<Buffer> {
  const img = await loadImage(input);
  const srcW = img.width;
  const srcH = img.height;
  const padding = opts.padding ?? 200;
  const background = opts.background ?? [200, 195, 185];
  const outW = srcW + padding * 2;
  const outH = srcH + padding * 2;

  // 元画像のピクセルを取得
  const srcCanvas = createCanvas(srcW, srcH);
  const srcCtx = srcCanvas.getContext('2d');
  srcCtx.drawImage(img, 0, 0);
  const src = srcCtx.getImageData(0, 0, srcW, srcH).data;

  // 出力4隅 → 元画像4隅 の逆方向ホモグラフィを解く
  const dstCorners = computeDistortedCorners(srcW, srcH, outW, outH, opts);
  const srcCorners: Point[] = [
    { x: 0, y: 0 },
    { x: srcW - 1, y: 0 },
    { x: 0, y: srcH - 1 },
    { x: srcW - 1, y: srcH - 1 },
  ];
  const hInv = computeHomography(dstCorners, srcCorners);

  const outCanvas = createCanvas(outW, outH);
  const outCtx = outCanvas.getContext('2d');
  const outImage = outCtx.createImageData(outW, outH);
  const out = outImage.data;

  const grad = opts.brightnessGradient ?? 0;
  const [bgR, bgG, bgB] = background;

  for (let dy = 0; dy < outH; dy++) {
    // 明度ムラ: 上端 1+grad/2 → 下端 1-grad/2 の線形勾配
    const brightness = grad === 0 ? 1 : 1 + grad * (0.5 - dy / (outH - 1));
    for (let dx = 0; dx < outW; dx++) {
      const o = (dy * outW + dx) * 4;
      const { x: sx, y: sy } = applyHomography(hInv, dx, dy);

      let r = bgR;
      let g = bgG;
      let b = bgB;
      if (sx >= 0 && sx <= srcW - 1 && sy >= 0 && sy <= srcH - 1) {
        // バイリニア補間
        const x0 = Math.floor(sx);
        const y0 = Math.floor(sy);
        const x1 = Math.min(x0 + 1, srcW - 1);
        const y1 = Math.min(y0 + 1, srcH - 1);
        const fx = sx - x0;
        const fy = sy - y0;
        const i00 = (y0 * srcW + x0) * 4;
        const i10 = (y0 * srcW + x1) * 4;
        const i01 = (y1 * srcW + x0) * 4;
        const i11 = (y1 * srcW + x1) * 4;
        const w00 = (1 - fx) * (1 - fy);
        const w10 = fx * (1 - fy);
        const w01 = (1 - fx) * fy;
        const w11 = fx * fy;
        r = src[i00] * w00 + src[i10] * w10 + src[i01] * w01 + src[i11] * w11;
        g = src[i00 + 1] * w00 + src[i10 + 1] * w10 + src[i01 + 1] * w01 + src[i11 + 1] * w11;
        b = src[i00 + 2] * w00 + src[i10 + 2] * w10 + src[i01 + 2] * w01 + src[i11 + 2] * w11;
      }

      out[o] = clamp255(r * brightness);
      out[o + 1] = clamp255(g * brightness);
      out[o + 2] = clamp255(b * brightness);
      out[o + 3] = 255;
    }
  }

  if (opts.blur) {
    boxBlur3x3(out, outW, outH);
  }

  outCtx.putImageData(outImage, 0, 0);
  return outCanvas.toBuffer('image/png');
}

function clamp255(v: number): number {
  return v < 0 ? 0 : v > 255 ? 255 : Math.round(v);
}

/** 3x3 ボックスぼかし（RGBのみ、in-place） */
function boxBlur3x3(data: Uint8ClampedArray, w: number, h: number): void {
  const srcCopy = new Uint8ClampedArray(data);
  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      for (let c = 0; c < 3; c++) {
        let sum = 0;
        for (let ky = -1; ky <= 1; ky++) {
          for (let kx = -1; kx <= 1; kx++) {
            sum += srcCopy[((y + ky) * w + (x + kx)) * 4 + c];
          }
        }
        data[(y * w + x) * 4 + c] = Math.round(sum / 9);
      }
    }
  }
}

/**
 * 歪みバリアント定義。generate-mock-scans.ts と e2e の両方から参照する
 * （ファイル名 suffix とパラメータの二重管理を避ける）。
 */
export const DISTORT_VARIANTS: { suffix: string; opts: DistortOptions }[] = [
  {
    // 台形変形（cli/src/distort.rs のデフォルトと同強度）
    suffix: 'perspective',
    opts: { trapezoid: 0.05, padding: 200 },
  },
  {
    // 数度の回転
    suffix: 'rotated',
    opts: { rotateDeg: 3, padding: 200 },
  },
  {
    // 複合: 軽い回転 + 軽い台形 + 縮小 + 明度ムラ + 軽いぼかし
    suffix: 'combined',
    opts: {
      rotateDeg: 2,
      trapezoid: 0.03,
      scale: 0.92,
      padding: 200,
      brightnessGradient: 0.1,
      blur: true,
    },
  },
];
