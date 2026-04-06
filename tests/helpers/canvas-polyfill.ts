/**
 * node-canvas を使って、ブラウザの Canvas API / createImageBitmap を
 * Node.js 環境でポリフィルする。
 * テストファイルの先頭で import するだけで有効になる。
 */

import { createCanvas, loadImage } from 'canvas';
import canvas_module from 'canvas';

// document.createElement('canvas') をフック
if (typeof globalThis.document === 'undefined') {
  (globalThis as any).document = {};
}
(globalThis as any).document.createElement = (tag: string) => {
  if (tag === 'canvas') {
    return createCanvas(1, 1) as any;
  }
  throw new Error(`document.createElement('${tag}') is not polyfilled`);
};

// createImageBitmap のポリフィル（Blob/File → node-canvas Image）
if (typeof globalThis.createImageBitmap === 'undefined') {
  (globalThis as any).createImageBitmap = async (source: Blob | File) => {
    const arrayBuffer = await source.arrayBuffer();
    const buffer = Buffer.from(arrayBuffer);
    const img = await loadImage(buffer);
    return {
      width: img.width,
      height: img.height,
      close() { /* noop */ },
      _img: img,
    };
  };
}

// CanvasRenderingContext2D.drawImage が polyfilled ImageBitmap を処理できるようパッチ
const { CanvasRenderingContext2D } = canvas_module as any;
const origDrawImage = CanvasRenderingContext2D.prototype.drawImage;
CanvasRenderingContext2D.prototype.drawImage = function (img: any, ...args: any[]) {
  const actualImg = img?._img ?? img;
  return origDrawImage.call(this, actualImg, ...args);
};

export { createCanvas, loadImage };
