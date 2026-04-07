import jsQR from 'jsqr';

export interface QRPayload {
  p: string; // project ("mfc")
  v: number; // version
  pg: number; // page number
  t: number; // total pages
  m: number; // cells per char
  chars?: string[]; // 明示的な文字リスト（リトライテンプレート用）
}

export function readQRFromImageData(data: ImageData): QRPayload | null {
  const code = jsQR(data.data, data.width, data.height);
  if (!code) return null;

  try {
    const payload = JSON.parse(code.data);
    if (typeof payload !== 'object' || payload === null) return null;
    if (payload.p !== 'mfc') return null;
    if (
      typeof payload.v !== 'number' ||
      typeof payload.pg !== 'number' ||
      typeof payload.t !== 'number'
    )
      return null;
    return payload as QRPayload;
  } catch {
    return null;
  }
}

// 画像のフッター領域からQRコードを探す（フッター領域を切り出して高速化）
export function readQRFromCanvas(canvas: HTMLCanvasElement): QRPayload | null {
  const ctx = canvas.getContext('2d');
  if (!ctx) return null;

  // まずフッター領域（下部20%）を試行
  const footerHeight = Math.floor(canvas.height * 0.2);
  const footerY = canvas.height - footerHeight;
  const footerData = ctx.getImageData(0, footerY, canvas.width, footerHeight);
  const result = readQRFromImageData(footerData);
  if (result) return result;

  // 見つからなければ全体を探索
  const fullData = ctx.getImageData(0, 0, canvas.width, canvas.height);
  return readQRFromImageData(fullData);
}
