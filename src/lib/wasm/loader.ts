/**
 * WASM ローダー: mfc (Rust) の WASM モジュールを初期化・管理する
 *
 * wasm-pack --target web で生成された mfc.js / mfc_bg.wasm を読み込む。
 * シングルトンで管理し、複数回呼ばれても初期化は1回だけ。
 */

// wasm-pack が生成するモジュールの型定義
// 実際のファイルは cli/ で wasm-pack build 後に src/wasm/ に生成される
interface MfcWasm {
  default: (input?: string | URL | ArrayBuffer) => Promise<void>;
  process_image: (image_bytes: Uint8Array) => unknown;
}

/** Rust側の ProcessedCell に対応 */
export interface WasmProcessedCell {
  row: number;
  col: number;
  char_index: number | null;
  is_empty: boolean;
  adopted: boolean;
  cell_index: number;
  image_data: number[]; // RGBA raw bytes
  width: number;
  height: number;
}

/** Rust側の ProcessResult に対応 */
export interface WasmProcessResult {
  page_number: number | null;
  total_pages: number | null;
  cells: WasmProcessedCell[];
  corrected_image: number[]; // RGBA raw bytes
  corrected_width: number;
  corrected_height: number;
}

let wasmModule: MfcWasm | null = null;
let initPromise: Promise<void> | null = null;

/**
 * WASMモジュールを初期化する（シングルトン）
 * 初回呼び出し時にのみfetch+instantiateを行う
 */
export async function initWasm(): Promise<void> {
  if (wasmModule) return;
  if (initPromise) {
    await initPromise;
    return;
  }

  initPromise = (async () => {
    // wasm-pack --target web --out-dir ../src/wasm で生成されるファイル
    const mod = (await import('../../wasm/mfc.js')) as unknown as MfcWasm;
    await mod.default();
    wasmModule = mod;
  })();

  await initPromise;
}

/**
 * WASM経由で画像を処理する
 * @param file 画像ファイル（JPEG/PNG）
 * @returns ProcessResult
 */
export async function processImageWasm(file: File): Promise<WasmProcessResult> {
  await initWasm();
  if (!wasmModule) throw new Error('WASM module not initialized');

  const buffer = await file.arrayBuffer();
  const bytes = new Uint8Array(buffer);
  const result = wasmModule.process_image(bytes) as WasmProcessResult;
  return result;
}

/**
 * WasmProcessedCell の image_data (RGBA配列) を ImageData に変換する
 */
export function cellToImageData(cell: WasmProcessedCell): ImageData {
  const data = new Uint8ClampedArray(cell.image_data);
  return new ImageData(data, cell.width, cell.height);
}

/**
 * WasmProcessedCell の image_data を Data URL (PNG) に変換する
 */
export function cellToDataUrl(cell: WasmProcessedCell): string {
  const imageData = cellToImageData(cell);
  const canvas = document.createElement('canvas');
  canvas.width = cell.width;
  canvas.height = cell.height;
  const ctx = canvas.getContext('2d')!;
  ctx.putImageData(imageData, 0, 0);
  return canvas.toDataURL('image/png');
}
