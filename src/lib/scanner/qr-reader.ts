import jsQR from 'jsqr';

export interface QRPayload {
  p: string;   // project ("mfc")
  v: number;   // version
  pg: number;  // page number
  t: number;   // total pages
  c: string[]; // chars on this page
  m: number;   // cells per char
}

export function readQRFromImageData(data: ImageData): QRPayload | null {
  const code = jsQR(data.data, data.width, data.height);
  if (!code) return null;

  try {
    const payload = JSON.parse(code.data) as QRPayload;
    if (payload.p !== 'mfc') return null;
    return payload;
  } catch {
    return null;
  }
}

// 画像の上部からQRコードを探す（ヘッダー領域を切り出して高速化）
export function readQRFromCanvas(canvas: HTMLCanvasElement): QRPayload | null {
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  // まずヘッダー領域（上部20%）を試行
  const headerHeight = Math.floor(canvas.height * 0.2);
  const headerData = ctx.getImageData(0, 0, canvas.width, headerHeight);
  const result = readQRFromImageData(headerData);
  if (result) return result;

  // 見つからなければ全体を探索
  const fullData = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return readQRFromImageData(fullData);
}
