/**
 * WASM ローダー: mfc (Rust) の WASM モジュールを初期化・管理する
 *
 * wasm-pack --target web で生成された mfc.js / mfc_bg.wasm を読み込む。
 * シングルトンで管理し、複数回呼ばれても初期化は1回だけ。
 */

import { createSignal } from 'solid-js';

// wasm-pack が生成するモジュールの型定義
// 実際のファイルは cli/ で wasm-pack build 後に src/wasm/ に生成される
interface MfcWasm {
  default: (input?: string | URL | ArrayBuffer) => Promise<void>;
  process_image: (image_bytes: Uint8Array) => unknown;
  build_info: () => string;
}

/** Rust 側 build_info() の JSON 形式（unixTs は秒単位の文字列） */
export interface WasmBuildInfo {
  sha: string;
  unixTs: string;
}

/**
 * ビルド識別情報の Solid シグナル（意図的にモジュール初期化時の「グローバル」配置）。
 *
 * Footer 等が購読するだけで WASM ロードをトリガーしないようにするため、
 * コンポーネント内の `createSignal` ではなくモジュールスコープで作っている。
 * `initWasm()` が成功したタイミングで `setWasmBuildInfo()` される。
 * owner 外の createSignal だが、グローバルシグナルの一般的パターンで Solid 上も問題なく動作する。
 */
export const [wasmBuildInfo, setWasmBuildInfo] = createSignal<WasmBuildInfo | null>(null);

/** 現在のビルド識別情報のスナップショットを返す（未初期化なら null） */
export function getWasmBuildInfo(): WasmBuildInfo | null {
  return wasmBuildInfo();
}

/**
 * Rust 側 vectorizer::PathCommand と一致するベジェコマンド型
 * serde の #[serde(tag = "type", rename = "M"|"L"|"C"|"Z")] で生成される
 */
export type WasmPathCommand =
  | { type: 'M'; x: number; y: number }
  | { type: 'L'; x: number; y: number }
  | {
      type: 'C';
      x: number;
      y: number;
      cp1x: number;
      cp1y: number;
      cp2x: number;
      cp2y: number;
    }
  | { type: 'Z'; x: number; y: number };

/** Rust側の CellQuality（#110: セル品質ゲートの結果）に対応 */
export interface WasmCellQuality {
  /** 除去した連結成分の数（境界接触 + 微小スペック） */
  removed_components: number;
  /** 除去した黒画素のセル全画素に対する比率 */
  removed_area_ratio: number;
  /** ゲート通過後に残った黒連結成分の数 */
  kept_components: number;
  /** ゲート通過後のインク率（黒画素 / セル全画素） */
  ink_ratio: number;
  /** 要確認フラグ。真なら review UI で「要確認」として見せる */
  needs_review: boolean;
}

/** Rust側の ProcessedCell に対応 */
export interface WasmProcessedCell {
  row: number;
  col: number;
  char_index: number | null;
  is_empty: boolean;
  adopted: boolean;
  cell_index: number;
  image_data: number[]; // RGBA raw bytes（二値化済み: 白背景+黒ストローク）
  width: number;
  height: number;
  /** 採用セルに対して Rust 側で生成されたベジェパス（輪郭単位の配列） */
  paths: WasmPathCommand[][];
  /** セル品質ゲートの結果（#110） */
  quality: WasmCellQuality;
}

/** Rust側の ProcessResult に対応 */
export interface WasmProcessResult {
  page_number: number | null;
  total_pages: number | null;
  /** 文字セット選択フラグ（Issue #91, v:3）。'h'/'k'/'a'/'j' の結合。null は QR 復元不可 */
  char_selection: string | null;
  /**
   * QR ペイロードの `chars` 配列（Issue #96, リトライ用 PDF のみ）。
   * 非 null かつ非空ならこれを優先して文字リストとして使う。
   */
  qr_chars: string[] | null;
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
    try {
      const info = JSON.parse(mod.build_info()) as WasmBuildInfo;
      setWasmBuildInfo(info);
      const ts = new Date(Number(info.unixTs) * 1000).toISOString();
      console.info(`[mfc] WASM build sha=${info.sha} built=${ts}`);
    } catch (e) {
      console.warn('[mfc] build_info() の取得に失敗:', e);
    }
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

/**
 * ベジェパスを SVG Data URL に変換する（TTF に近い見た目のプレビュー用）
 * 座標系: Rust 側 normalize_contour は x ∈ [0, UNITS_PER_EM], y ∈ [0, GLYPH_HEIGHT] で
 *   font 座標（y 上向き）を出力する。SVG は y 下向きなので反転する。
 * viewBox は GLYPH_HEIGHT に揃えてグリフ本体を枠いっぱいに配置する
 */
export function pathsToSvgDataUrl(paths: WasmPathCommand[][]): string {
  const UNITS_PER_EM = 1000;
  const GLYPH_HEIGHT = 800;
  const flipY = (y: number) => GLYPH_HEIGHT - y;
  const d: string[] = [];
  for (const sub of paths) {
    for (const c of sub) {
      switch (c.type) {
        case 'M':
          d.push(`M${c.x},${flipY(c.y)}`);
          break;
        case 'L':
          d.push(`L${c.x},${flipY(c.y)}`);
          break;
        case 'C':
          d.push(`C${c.cp1x},${flipY(c.cp1y)} ${c.cp2x},${flipY(c.cp2y)} ${c.x},${flipY(c.y)}`);
          break;
        case 'Z':
          d.push('Z');
          break;
      }
    }
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${UNITS_PER_EM} ${GLYPH_HEIGHT}" preserveAspectRatio="xMidYMid meet"><path d="${d.join(' ')}" fill="black" fill-rule="evenodd"/></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
