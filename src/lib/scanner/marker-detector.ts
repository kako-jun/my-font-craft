// 四隅マーカー検出と台形補正

import { PAGE_WIDTH, PAGE_HEIGHT, MARKERS, MARKER_SIZE } from '../template/layout';

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
// 連結成分解析で個別ブロブを識別し、マーカーらしいもの（コンパクトで十分なサイズ）を選ぶ
export function detectMarkers(canvas: HTMLCanvasElement): Corners | null {
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  const w = canvas.width;
  const h = canvas.height;
  const imgData = ctx.getImageData(0, 0, w, h);
  const data = imgData.data;

  const threshold = computeOtsuThreshold(data, w * h);

  // 四隅の領域で最大のコンパクトなブロブ（＝マーカー）を見つける
  const margin = Math.floor(Math.min(w, h) * 0.15);
  const topLeft = findMarkerBlob(data, w, h, 0, 0, margin, margin, threshold);
  const topRight = findMarkerBlob(data, w, h, w - margin, 0, margin, margin, threshold);
  const bottomLeft = findMarkerBlob(data, w, h, 0, h - margin, margin, margin, threshold);
  const bottomRight = findMarkerBlob(data, w, h, w - margin, h - margin, margin, margin, threshold);

  if (!topLeft || !topRight || !bottomLeft || !bottomRight) return null;

  return { topLeft, topRight, bottomLeft, bottomRight };
}

interface Blob {
  cx: number;
  cy: number;
  area: number;
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

// 連結成分ラベリング（4連結）でブロブを見つけ、マーカーらしいものを返す
function findMarkerBlob(
  data: Uint8ClampedArray,
  imgW: number,
  imgH: number,
  rx: number,
  ry: number,
  rw: number,
  rh: number,
  threshold: number,
): Point | null {
  const endX = Math.min(rx + rw, imgW);
  const endY = Math.min(ry + rh, imgH);
  const regionW = endX - rx;
  const regionH = endY - ry;

  // 領域内の二値画像を作成
  const binary = new Uint8Array(regionW * regionH);
  for (let y = 0; y < regionH; y++) {
    for (let x = 0; x < regionW; x++) {
      const i = ((ry + y) * imgW + (rx + x)) * 4;
      const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
      binary[y * regionW + x] = gray < threshold ? 1 : 0;
    }
  }

  // 連結成分ラベリング（4連結、union-find）
  const labels = new Int32Array(regionW * regionH);
  labels.fill(-1);
  const parent = new Int32Array(regionW * regionH);
  for (let i = 0; i < parent.length; i++) parent[i] = i;

  function find(a: number): number {
    while (parent[a] !== a) {
      parent[a] = parent[parent[a]];
      a = parent[a];
    }
    return a;
  }
  function union(a: number, b: number) {
    const ra = find(a);
    const rb = find(b);
    if (ra !== rb) parent[rb] = ra;
  }

  let nextLabel = 0;
  for (let y = 0; y < regionH; y++) {
    for (let x = 0; x < regionW; x++) {
      const idx = y * regionW + x;
      if (!binary[idx]) continue;

      const up = y > 0 ? labels[(y - 1) * regionW + x] : -1;
      const left = x > 0 ? labels[y * regionW + (x - 1)] : -1;

      if (up >= 0 && left >= 0) {
        labels[idx] = up;
        union(up, left);
      } else if (up >= 0) {
        labels[idx] = up;
      } else if (left >= 0) {
        labels[idx] = left;
      } else {
        labels[idx] = nextLabel++;
      }
    }
  }

  // ブロブ統計を集計
  const blobMap = new Map<number, Blob>();
  for (let y = 0; y < regionH; y++) {
    for (let x = 0; x < regionW; x++) {
      const idx = y * regionW + x;
      if (labels[idx] < 0) continue;
      const root = find(labels[idx]);
      let blob = blobMap.get(root);
      if (!blob) {
        blob = { cx: 0, cy: 0, area: 0, minX: x, minY: y, maxX: x, maxY: y };
        blobMap.set(root, blob);
      }
      blob.cx += rx + x;
      blob.cy += ry + y;
      blob.area++;
      blob.minX = Math.min(blob.minX, x);
      blob.minY = Math.min(blob.minY, y);
      blob.maxX = Math.max(blob.maxX, x);
      blob.maxY = Math.max(blob.maxY, y);
    }
  }

  // マーカーらしいブロブを選別:
  // - 面積が十分（ノイズ排除）: 最低100px
  // - コンパクト（格子線排除）: 幅と高さの比率が0.3〜3.0
  // - 充填率が高い（格子線の交差排除）: 面積/(幅*高さ) > 0.15
  const candidates: Blob[] = [];
  for (const blob of blobMap.values()) {
    const blobW = blob.maxX - blob.minX + 1;
    const blobH = blob.maxY - blob.minY + 1;
    const aspect = blobW / blobH;
    const fillRatio = blob.area / (blobW * blobH);

    if (blob.area >= 100 && aspect >= 0.3 && aspect <= 3.0 && fillRatio > 0.15) {
      blob.cx /= blob.area;
      blob.cy /= blob.area;
      candidates.push(blob);
    }
  }

  if (candidates.length === 0) return null;

  // 最大面積のブロブを選択（マーカーは領域内で最大のコンパクトなブロブのはず）
  candidates.sort((a, b) => b.area - a.area);
  const best = candidates[0];
  return { x: best.cx, y: best.cy };
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

  let sumB = 0,
    wB = 0;
  let maxVariance = 0,
    bestThreshold = 128;

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

// ���ャンバスを指定角度（90/180/270）で回転
export function rotateCanvas(canvas: HTMLCanvasElement, degrees: number): HTMLCanvasElement {
  const dst = document.createElement('canvas');
  const swap = degrees === 90 || degrees === 270;
  dst.width = swap ? canvas.height : canvas.width;
  dst.height = swap ? canvas.width : canvas.height;
  const ctx = dst.getContext('2d')!;
  ctx.translate(dst.width / 2, dst.height / 2);
  ctx.rotate((degrees * Math.PI) / 180);
  ctx.drawImage(canvas, -canvas.width / 2, -canvas.height / 2);
  return dst;
}

// マーカー位置からページ全体の四隅を外挿する
// マーカーはページ内の既知位置にあるため、比率から外側のページ端を算出

export function extrapolatePageCorners(markerCorners: Corners): Corners {
  // マーカー中心のページ内座標（mm）
  const mLeft = MARKERS.topLeft.x + MARKER_SIZE / 2;
  const mRight = MARKERS.topRight.x + MARKER_SIZE / 2;
  const mTop = MARKERS.topLeft.y + MARKER_SIZE / 2;
  const mBottom = MARKERS.bottomLeft.y + MARKER_SIZE / 2;

  // マーカー間の比率からページ端を線形外挿
  const ml = markerCorners.topLeft;
  const mr = markerCorners.topRight;
  const bl = markerCorners.bottomLeft;
  const br = markerCorners.bottomRight;

  // 水平方向: マーカー左端→右端の間隔から、ページ左端・右端を外挿
  const ratioLeft = mLeft / (mRight - mLeft);
  const ratioRight = (PAGE_WIDTH - mRight) / (mRight - mLeft);
  // 垂直方向: マーカー上端→下端の間隔から、ページ上端・下端を外挿
  const ratioTop = mTop / (mBottom - mTop);
  const ratioBottom = (PAGE_HEIGHT - mBottom) / (mBottom - mTop);

  return {
    topLeft: {
      x: ml.x - (mr.x - ml.x) * ratioLeft,
      y: ml.y - (bl.y - ml.y) * ratioTop,
    },
    topRight: {
      x: mr.x + (mr.x - ml.x) * ratioRight,
      y: mr.y - (br.y - mr.y) * ratioTop,
    },
    bottomLeft: {
      x: bl.x - (br.x - bl.x) * ratioLeft,
      y: bl.y + (bl.y - ml.y) * ratioBottom,
    },
    bottomRight: {
      x: br.x + (br.x - bl.x) * ratioRight,
      y: br.y + (br.y - mr.y) * ratioBottom,
    },
  };
}

// 射影変換（4点→長方形）— 最近傍法による逆変換
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

  // 逆変換（最近傍法）
  for (let dy = 0; dy < targetHeight; dy++) {
    for (let dx = 0; dx < targetWidth; dx++) {
      const u = dx / targetWidth;
      const v = dy / targetHeight;

      // 双線形マッピングで元画像の座標を算出
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

// 向き検出: 塗りつぶしマーカー（黒ピクセル密度が最も高い）を左上と判定
export function detectOrientation(corners: Corners, canvas: HTMLCanvasElement): number {
  const ctx = canvas.getContext('2d')!;
  const w = canvas.width;
  const h = canvas.height;

  // 二値化閾値を算出（固定閾値128をフォールバックに使用）
  const fullData = ctx.getImageData(0, 0, w, h).data;
  const threshold = computeOtsuThreshold(fullData, w * h);

  // マーカーの画像上のサイズを推定（ページ幅に対するマーカーサイズの比率から）
  const dx = corners.topRight.x - corners.topLeft.x;
  const markerPixelSize = (MARKER_SIZE / (MARKERS.topRight.x - MARKERS.topLeft.x)) * dx;
  const r = Math.max(10, Math.round(markerPixelSize / 2));

  // マーカー周辺の黒ピクセル密度を計測（塗りつぶし vs 枠線のみ を区別）
  function blackPixelDensity(point: Point): number {
    const px = Math.max(0, Math.min(Math.round(point.x), w - 1));
    const py = Math.max(0, Math.min(Math.round(point.y), h - 1));
    const x0 = Math.max(0, px - r);
    const y0 = Math.max(0, py - r);
    const x1 = Math.min(w, px + r);
    const y1 = Math.min(h, py + r);
    const data = ctx.getImageData(x0, y0, x1 - x0, y1 - y0).data;
    let blackCount = 0;
    const totalPixels = data.length / 4;
    for (let i = 0; i < data.length; i += 4) {
      const gray = (data[i] + data[i + 1] + data[i + 2]) / 3;
      if (gray < threshold) {
        blackCount++;
      }
    }
    return totalPixels > 0 ? blackCount / totalPixels : 0;
  }

  const cornerEntries = [
    { corner: 'topLeft', point: corners.topLeft },
    { corner: 'topRight', point: corners.topRight },
    { corner: 'bottomLeft', point: corners.bottomLeft },
    { corner: 'bottomRight', point: corners.bottomRight },
  ] as const;

  // 密度方式: 黒ピクセル密度が最も高いマーカーを topLeft と判定
  const densities = cornerEntries.map((c) => ({
    corner: c.corner,
    val: blackPixelDensity(c.point),
  }));
  const densest = densities.reduce((a, b) => (a.val > b.val ? a : b));

  function cornerToRotation(corner: 'topLeft' | 'topRight' | 'bottomLeft' | 'bottomRight'): number {
    switch (corner) {
      case 'topLeft':
        return 0;
      case 'topRight':
        return 90;
      case 'bottomRight':
        return 180;
      case 'bottomLeft':
        return 270;
      default:
        return 0;
    }
  }

  return cornerToRotation(densest.corner);
}
