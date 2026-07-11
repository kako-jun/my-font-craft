/**
 * pathsToSvgDataUrl の座標系固定テスト（#111 QA）。
 *
 * loader.ts はセル crop の em 範囲（CROP_X_MIN 等の4定数）をハードコードしており、
 * Rust 側 vectorizer::crop_em_bounds()（= (-100, -220, 1100, 980)、
 * crop_em_bounds_matches_layout テストで固定）との drift をここで検知する。
 * どちらかを変えたら両方のテストが落ちる。
 */

import { describe, it, expect } from 'vitest';
import { pathsToSvgDataUrl } from '../../src/lib/wasm/loader';
import type { WasmPathCommand } from '../../src/lib/wasm/loader';

describe('pathsToSvgDataUrl: セル→em 固定変換の viewBox (#111)', () => {
  const paths: WasmPathCommand[][] = [
    [
      { type: 'M', x: 0, y: 980 }, // crop 上端（em y_max）
      { type: 'L', x: 100, y: -220 }, // crop 下端（em y_min）
      { type: 'Z', x: 0, y: 980 },
    ],
  ];

  it('viewBox がセル crop 全域 "-100 0 1200 1200" に固定されている', () => {
    const url = pathsToSvgDataUrl(paths);
    expect(url.startsWith('data:image/svg+xml;utf8,')).toBe(true);
    const svg = decodeURIComponent(url.slice('data:image/svg+xml;utf8,'.length));
    // Rust vectorizer::crop_em_bounds() と一致させること（drift 検知）
    expect(svg).toContain('viewBox="-100 0 1200 1200"');
  });

  it('Y 反転が crop 上端/下端を viewBox の 0/1200 に写す', () => {
    const url = pathsToSvgDataUrl(paths);
    const svg = decodeURIComponent(url.slice('data:image/svg+xml;utf8,'.length));
    // font y=980（crop 上端）→ svg y=0、font y=-220（crop 下端）→ svg y=1200
    expect(svg).toContain('M0,0');
    expect(svg).toContain('L100,1200');
  });
});
