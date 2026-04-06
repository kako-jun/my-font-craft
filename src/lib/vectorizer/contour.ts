// 輪郭抽出・ベクター化モジュール

export interface PathCommand {
  type: 'M' | 'L' | 'C' | 'Z';
  x: number;
  y: number;
  cp1x?: number;
  cp1y?: number;
  cp2x?: number;
  cp2y?: number;
}

const UNITS_PER_EM = 1000;
const GLYPH_HEIGHT = 800;

// ImageDataからベクターパスを生成
export function vectorizeGlyph(imageData: ImageData): PathCommand[][] {
  // 1. 二値化
  const binary = binarize(imageData);

  // 2. 輪郭抽出
  const contours = extractContours(binary, imageData.width, imageData.height);

  // 3. Douglas-Peucker簡略化
  const simplified = contours.map(c => douglasPeucker(c, 1.0));

  // 4. 正規化（画像座標→フォント座標系）
  const normalized = simplified.map(c => normalizeContour(c, imageData.width, imageData.height));

  // 5. ベジェ曲線変換
  return normalized.map(contourToPath);
}

// 大津の方法で二値化
function binarize(imageData: ImageData): Uint8Array {
  const { data, width, height } = imageData;
  const gray = new Uint8Array(width * height);
  const histogram = new Uint32Array(256);

  for (let i = 0; i < width * height; i++) {
    const idx = i * 4;
    const g = Math.round((data[idx] + data[idx + 1] + data[idx + 2]) / 3);
    gray[i] = g;
    histogram[g]++;
  }

  // 大津の閾値
  const total = width * height;
  let sumAll = 0;
  for (let i = 0; i < 256; i++) sumAll += i * histogram[i];

  let sumB = 0, wB = 0, maxVar = 0, threshold = 128;
  for (let t = 0; t < 256; t++) {
    wB += histogram[t];
    if (wB === 0) continue;
    const wF = total - wB;
    if (wF === 0) break;
    sumB += t * histogram[t];
    const meanB = sumB / wB;
    const meanF = (sumAll - sumB) / wF;
    const v = wB * wF * (meanB - meanF) ** 2;
    if (v > maxVar) { maxVar = v; threshold = t; }
  }

  const result = new Uint8Array(width * height);
  for (let i = 0; i < gray.length; i++) {
    result[i] = gray[i] < threshold ? 1 : 0;
  }
  return result;
}

interface Pt { x: number; y: number }

// 簡易輪郭抽出（境界追跡）
function extractContours(binary: Uint8Array, w: number, h: number): Pt[][] {
  const visited = new Uint8Array(w * h);
  const contours: Pt[][] = [];

  // 8方向
  const dx = [1, 1, 0, -1, -1, -1, 0, 1];
  const dy = [0, 1, 1, 1, 0, -1, -1, -1];

  for (let y = 1; y < h - 1; y++) {
    for (let x = 1; x < w - 1; x++) {
      const idx = y * w + x;
      if (binary[idx] !== 1 || visited[idx]) continue;

      // 境界ピクセルかチェック
      let isBorder = false;
      for (let d = 0; d < 8; d++) {
        const nx = x + dx[d];
        const ny = y + dy[d];
        if (binary[ny * w + nx] === 0) { isBorder = true; break; }
      }
      if (!isBorder) continue;

      // 境界追跡
      const contour: Pt[] = [];
      let cx = x, cy = y;
      let dir = 0;
      const startX = x, startY = y;
      let steps = 0;
      const maxSteps = w * h;

      do {
        contour.push({ x: cx, y: cy });
        visited[cy * w + cx] = 1;

        let found = false;
        for (let i = 0; i < 8; i++) {
          const nd = (dir + 6 + i) % 8; // 左回りに探索
          const nx = cx + dx[nd];
          const ny = cy + dy[nd];
          if (nx >= 0 && nx < w && ny >= 0 && ny < h && binary[ny * w + nx] === 1) {
            // 境界ピクセルか確認
            let nb = false;
            for (let d2 = 0; d2 < 8; d2++) {
              const nnx = nx + dx[d2];
              const nny = ny + dy[d2];
              if (nnx >= 0 && nnx < w && nny >= 0 && nny < h && binary[nny * w + nnx] === 0) {
                nb = true; break;
              }
            }
            if (nb) {
              cx = nx; cy = ny; dir = nd;
              found = true;
              break;
            }
          }
        }

        if (!found) break;
        steps++;
      } while ((cx !== startX || cy !== startY) && steps < maxSteps);

      // 小さすぎる輪郭は除外（ノイズ）
      if (contour.length >= 10) {
        contours.push(contour);
      }
    }
  }

  return contours;
}

// Douglas-Peucker アルゴリズム
function douglasPeucker(points: Pt[], epsilon: number): Pt[] {
  if (points.length <= 2) return points;

  let maxDist = 0;
  let maxIdx = 0;
  const start = points[0];
  const end = points[points.length - 1];

  for (let i = 1; i < points.length - 1; i++) {
    const d = perpendicularDist(points[i], start, end);
    if (d > maxDist) { maxDist = d; maxIdx = i; }
  }

  if (maxDist > epsilon) {
    const left = douglasPeucker(points.slice(0, maxIdx + 1), epsilon);
    const right = douglasPeucker(points.slice(maxIdx), epsilon);
    return [...left.slice(0, -1), ...right];
  }

  return [start, end];
}

function perpendicularDist(p: Pt, a: Pt, b: Pt): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len = Math.sqrt(dx * dx + dy * dy);
  if (len === 0) return Math.sqrt((p.x - a.x) ** 2 + (p.y - a.y) ** 2);
  return Math.abs(dy * p.x - dx * p.y + b.x * a.y - b.y * a.x) / len;
}

// 画像座標→フォント座標系に正規化
function normalizeContour(points: Pt[], imgW: number, imgH: number): Pt[] {
  // フォント座標系: Y軸が上向き、unitsPerEm = 1000
  const scale = GLYPH_HEIGHT / imgH;
  const offsetX = (UNITS_PER_EM - imgW * scale) / 2;

  return points.map(p => ({
    x: Math.round(p.x * scale + offsetX),
    y: Math.round(GLYPH_HEIGHT - p.y * scale), // Y反転
  }));
}

// 輪郭点列→PathCommandに変換（ベジェ曲線近似）
function contourToPath(points: Pt[]): PathCommand[] {
  if (points.length < 2) return [];

  const commands: PathCommand[] = [];
  commands.push({ type: 'M', x: points[0].x, y: points[0].y });

  // 3点ずつでベジェ曲線を生成、余りは直線
  let i = 1;
  while (i < points.length) {
    if (i + 2 < points.length) {
      // 3点から2次ベジェを3次ベジェに昇格
      const p0 = points[i - 1];
      const p1 = points[i];
      const p2 = points[i + 1];
      const p3 = points[i + 2];

      commands.push({
        type: 'C',
        x: p3.x,
        y: p3.y,
        cp1x: p0.x + (p1.x - p0.x) * 0.66,
        cp1y: p0.y + (p1.y - p0.y) * 0.66,
        cp2x: p3.x + (p2.x - p3.x) * 0.66,
        cp2y: p3.y + (p2.y - p3.y) * 0.66,
      });
      i += 3;
    } else {
      commands.push({ type: 'L', x: points[i].x, y: points[i].y });
      i++;
    }
  }

  commands.push({ type: 'Z', x: points[0].x, y: points[0].y });
  return commands;
}
