/**
 * 合成スキャン画像の後処理（#113 実写ループ拡張）
 *
 * 正面レンダリング済みの PNG バッファに対し、実写に近いが回復可能な範囲の
 * 劣化を後処理として乗せる:
 * - ごま塩ノイズ（salt-and-pepper）+ 微小スペック
 * - 照明ムラ（明度グラデーション + 影の帯 + コントラスト低下）
 *
 * 罫線残渣・枠残渣は既存の drawCellResidue（generateResidueScans）が担うため
 * ここでは扱わない（重複回避）。台形/回転/縮小/ぼかしは distort.ts が担う。
 *
 * パラメータは「実写に近いが回復可能」に寄せる: 二値化・マーカー/QR 検出が
 * 破綻しない密度・強度に留め、複数の中程度フィクスチャで面を張る。
 */

import { createCanvas, loadImage } from 'canvas';

/** 決定的乱数（mulberry32）。フィクスチャ再現性のため seed 固定で使う。 */
export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export interface NoiseOptions {
  /** 各ピクセルが塩（白点）になる確率 */
  saltProb?: number;
  /** 各ピクセルが胡椒（黒点）になる確率 */
  pepperProb?: number;
  /** 追加で撒く微小スペック（1〜2px 塊）の個数 */
  speckCount?: number;
  /** 乱数シード */
  seed?: number;
}

/**
 * ごま塩ノイズ + 微小スペックを乗せる。
 * 密度は低め（既定 0.15% 前後）に留め、四隅マーカー（大きい塗り円）や
 * QR（誤り訂正 M）を壊さない範囲にする。
 */
export async function addSaltPepperNoise(input: Buffer, opts: NoiseOptions = {}): Promise<Buffer> {
  const saltProb = opts.saltProb ?? 0.0015;
  const pepperProb = opts.pepperProb ?? 0.0015;
  const speckCount = opts.speckCount ?? 400;
  const rand = mulberry32(opts.seed ?? 0x5a17);

  const img = await loadImage(input);
  const w = img.width;
  const h = img.height;
  const canvas = createCanvas(w, h);
  const ctx = canvas.getContext('2d');
  ctx.drawImage(img, 0, 0);
  const imageData = ctx.getImageData(0, 0, w, h);
  const d = imageData.data;

  for (let i = 0; i < w * h; i++) {
    const r = rand();
    if (r < pepperProb) {
      const o = i * 4;
      d[o] = d[o + 1] = d[o + 2] = 0;
    } else if (r < pepperProb + saltProb) {
      const o = i * 4;
      d[o] = d[o + 1] = d[o + 2] = 255;
    }
  }

  // 微小スペック: 1〜2px の黒/白の塊（ちり・ほこり）
  for (let s = 0; s < speckCount; s++) {
    const cx = Math.floor(rand() * w);
    const cy = Math.floor(rand() * h);
    const size = rand() < 0.75 ? 1 : 2;
    const black = rand() < 0.6;
    const v = black ? 0 : 255;
    for (let dy = 0; dy < size; dy++) {
      for (let dx = 0; dx < size; dx++) {
        const x = cx + dx;
        const y = cy + dy;
        if (x >= w || y >= h) continue;
        const o = (y * w + x) * 4;
        d[o] = d[o + 1] = d[o + 2] = v;
      }
    }
  }

  ctx.putImageData(imageData, 0, 0);
  return canvas.toBuffer('image/png');
}

export interface LightingOptions {
  /** 上下方向の明度グラデーション幅（例 0.25 → 上 +12.5% 〜 下 −12.5%） */
  gradient?: number;
  /** 影の帯の中心 y（0〜1 の相対位置）。未指定なら帯なし */
  shadowBandCenter?: number;
  /** 影の帯の相対高さ（0〜1） */
  shadowBandHeight?: number;
  /** 影の帯の暗さ（乗算係数, 例 0.75） */
  shadowBandStrength?: number;
  /** コントラスト低下係数（1=変化なし, 0.7=中央グレーへ 30% 寄せる） */
  contrast?: number;
}

/**
 * 照明ムラを乗せる: 明度グラデーション + 影の帯 + コントラスト低下。
 * 二値化がインク（黒）と紙（白）を分離できる強度に留める
 * （黒→コントラスト0.7でも ~38 で十分暗い）。
 */
export async function applyLighting(input: Buffer, opts: LightingOptions = {}): Promise<Buffer> {
  const gradient = opts.gradient ?? 0.25;
  const contrast = opts.contrast ?? 0.8;
  const bandCenter = opts.shadowBandCenter;
  const bandHeight = opts.shadowBandHeight ?? 0.12;
  const bandStrength = opts.shadowBandStrength ?? 0.78;

  const img = await loadImage(input);
  const w = img.width;
  const h = img.height;
  const canvas = createCanvas(w, h);
  const ctx = canvas.getContext('2d');
  ctx.drawImage(img, 0, 0);
  const imageData = ctx.getImageData(0, 0, w, h);
  const d = imageData.data;

  const bandY0 = bandCenter === undefined ? -1 : (bandCenter - bandHeight / 2) * h;
  const bandY1 = bandCenter === undefined ? -1 : (bandCenter + bandHeight / 2) * h;

  for (let y = 0; y < h; y++) {
    // 明度グラデーション: 上端 1+g/2 → 下端 1-g/2
    let mult = 1 + gradient * (0.5 - y / (h - 1));
    // 影の帯（乗算）
    if (bandCenter !== undefined && y >= bandY0 && y <= bandY1) {
      mult *= bandStrength;
    }
    for (let x = 0; x < w; x++) {
      const o = (y * w + x) * 4;
      for (let c = 0; c < 3; c++) {
        let v = d[o + c] * mult;
        // コントラスト低下: 中央グレー(128)へ寄せる
        v = 128 + (v - 128) * contrast;
        d[o + c] = v < 0 ? 0 : v > 255 ? 255 : Math.round(v);
      }
    }
  }

  ctx.putImageData(imageData, 0, 0);
  return canvas.toBuffer('image/png');
}
