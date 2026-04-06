// 四隅マーカー検出と台形補正

export interface Point {
  x: number;
  y: number;
}

export interface Corners {
  topLeft: Point;
  topRight: Point;
  bottomLeft: Point;
  bottomRight: Point;
}

// 二値化して黒い領域の塊（マーカー候補）を探す
export function detectMarkers(canvas: HTMLCanvasElement): Corners | null {
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  const w = canvas.width;
  const h = canvas.height;
  const imgData = ctx.getImageData(0, 0, w, h);
  const data = imgData.data;

  // 二値化閾値（大津の方法の簡易版）
  const threshold = computeOtsuThreshold(data, w * h);

  // 四隅の領域で黒い塊の重心を見つける
  const margin = Math.floor(Math.min(w, h) * 0.15);

  const topLeft = findMarkerInRegion(data, w, h, 0, 0, margin, margin, threshold);
  const topRight = findMarkerInRegion(data, w, h, w - margin, 0, margin, margin, threshold);
  const bottomLeft = findMarkerInRegion(data, w, h, 0, h - margin, margin, margin, threshold);
  const bottomRight = findMarkerInRegion(data, w, h, w - margin, h - margin, margin, margin, threshold);

  if (!topLeft || !topRight || !bottomLeft || !bottomRight) return null;

  return { topLeft, topRight, bottomLeft, bottomRight };
}

function findMarkerInRegion(
  data: Uint8ClampedArray,
  imgW: number, imgH: number,
  rx: number, ry: number, rw: number, rh: number,
  threshold: number,
): Point | null {
  let sumX = 0, sumY = 0, count = 0;

  for (let y = ry; y < Math.min(ry + rh, imgH); y++) {
    for (let x = rx; x < Math.min(rx + rw, imgW); x++) {
      const i = (y * imgW + x) * 4;
      const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
      if (gray < threshold) {
        sumX += x;
        sumY += y;
        count++;
      }
    }
  }

  if (count < 10) return null;
  return { x: sumX / count, y: sumY / count };
}

function computeOtsuThreshold(data: Uint8ClampedArray, pixelCount: number): number {
  const histogram = new Uint32Array(256);
  for (let i = 0; i < pixelCount; i++) {
    const idx = i * 4;
    const gray = Math.round((data[idx] + data[idx + 1] + data[idx + 2]) / 3);
    histogram[gray]++;
  }

  let total = pixelCount;
  let sumAll = 0;
  for (let i = 0; i < 256; i++) sumAll += i * histogram[i];

  let sumB = 0, wB = 0;
  let maxVariance = 0, bestThreshold = 128;

  for (let t = 0; t < 256; t++) {
    wB += histogram[t];
    if (wB === 0) continue;
    const wF = total - wB;
    if (wF === 0) break;

    sumB += t * histogram[t];
    const meanB = sumB / wB;
    const meanF = (sumAll - sumB) / wF;
    const variance = wB * wF * (meanB - meanF) * (meanB - meanF);

    if (variance > maxVariance) {
      maxVariance = variance;
      bestThreshold = t;
    }
  }

  return bestThreshold;
}

// 射影変換（4点→長方形）
export function perspectiveTransform(
  srcCanvas: HTMLCanvasElement,
  corners: Corners,
  targetWidth: number,
  targetHeight: number,
): HTMLCanvasElement {
  const dst = document.createElement('canvas');
  dst.width = targetWidth;
  dst.height = targetHeight;
  const dstCtx = dst.getContext('2d')!;

  const srcCtx = srcCanvas.getContext('2d')!;
  const srcData = srcCtx.getImageData(0, 0, srcCanvas.width, srcCanvas.height);
  const dstData = dstCtx.createImageData(targetWidth, targetHeight);

  // 双線形補間による逆変換
  for (let dy = 0; dy < targetHeight; dy++) {
    for (let dx = 0; dx < targetWidth; dx++) {
      const u = dx / targetWidth;
      const v = dy / targetHeight;

      // 双線形補間で元画像の座標を算出
      const srcX =
        (1 - u) * (1 - v) * corners.topLeft.x +
        u * (1 - v) * corners.topRight.x +
        (1 - u) * v * corners.bottomLeft.x +
        u * v * corners.bottomRight.x;
      const srcY =
        (1 - u) * (1 - v) * corners.topLeft.y +
        u * (1 - v) * corners.topRight.y +
        (1 - u) * v * corners.bottomLeft.y +
        u * v * corners.bottomRight.y;

      const sx = Math.round(srcX);
      const sy = Math.round(srcY);

      if (sx >= 0 && sx < srcCanvas.width && sy >= 0 && sy < srcCanvas.height) {
        const si = (sy * srcCanvas.width + sx) * 4;
        const di = (dy * targetWidth + dx) * 4;
        dstData.data[di] = srcData.data[si];
        dstData.data[di + 1] = srcData.data[si + 1];
        dstData.data[di + 2] = srcData.data[si + 2];
        dstData.data[di + 3] = srcData.data[si + 3];
      }
    }
  }

  dstCtx.putImageData(dstData, 0, 0);
  return dst;
}

// 向き検出: 左上マーカーが一番暗い（塗りつぶし）
export function detectOrientation(corners: Corners, canvas: HTMLCanvasElement): number {
  const ctx = canvas.getContext('2d')!;
  const w = canvas.width;
  const h = canvas.height;

  function avgBrightness(point: Point): number {
    const r = 10;
    const px = Math.max(0, Math.min(Math.round(point.x), w - 1));
    const py = Math.max(0, Math.min(Math.round(point.y), h - 1));
    const x0 = Math.max(0, px - r);
    const y0 = Math.max(0, py - r);
    const x1 = Math.min(w, px + r);
    const y1 = Math.min(h, py + r);
    const data = ctx.getImageData(x0, y0, x1 - x0, y1 - y0).data;
    let sum = 0;
    const count = data.length / 4;
    for (let i = 0; i < data.length; i += 4) {
      sum += (data[i] + data[i + 1] + data[i + 2]) / 3;
    }
    return sum / count;
  }

  const brightnesses = [
    { corner: 'topLeft', val: avgBrightness(corners.topLeft) },
    { corner: 'topRight', val: avgBrightness(corners.topRight) },
    { corner: 'bottomLeft', val: avgBrightness(corners.bottomLeft) },
    { corner: 'bottomRight', val: avgBrightness(corners.bottomRight) },
  ];

  const darkest = brightnesses.reduce((a, b) => a.val < b.val ? a : b);

  switch (darkest.corner) {
    case 'topLeft': return 0;
    case 'topRight': return 90;
    case 'bottomRight': return 180;
    case 'bottomLeft': return 270;
    default: return 0;
  }
}
