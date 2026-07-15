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

describe('pathsToSvgDataUrl: nonzero winding 統一 (#112)', () => {
  it('プレビューは nonzero で塗る（evenodd は使わない）', () => {
    // 本番 CFF フォント（opentype.js）と Rust paths_to_svg は nonzero。
    // プレビューが evenodd に退行すると自己交差ストロークで実フォントと食い違う。
    const paths: WasmPathCommand[][] = [
      [
        { type: 'M', x: 0, y: 0 },
        { type: 'L', x: 100, y: 0 },
        { type: 'L', x: 100, y: 100 },
        { type: 'Z', x: 0, y: 0 },
      ],
    ];
    const url = pathsToSvgDataUrl(paths);
    const svg = decodeURIComponent(url.slice('data:image/svg+xml;utf8,'.length));
    expect(svg).toContain('fill-rule="nonzero"');
    expect(svg).not.toContain('fill-rule="evenodd"');
  });

  it('自己交差パス（図形8）でもプレビューは font と同じ nonzero を使う', () => {
    // 図形8（自己交差）: 2つのループの巻き方向が同じ = nonzero では両方塗られるが
    // evenodd では交差で穴が開く。プレビューは font（nonzero）に一致させる。
    const figureEight: WasmPathCommand[][] = [
      [
        // 下ループ（CW）
        { type: 'M', x: 0, y: 0 },
        { type: 'L', x: 100, y: 0 },
        { type: 'L', x: 100, y: 100 },
        { type: 'L', x: 0, y: 100 },
        { type: 'Z', x: 0, y: 0 },
        // 上ループ（同じ CW・下ループと一点で交差）
        { type: 'M', x: 50, y: 100 },
        { type: 'L', x: 150, y: 100 },
        { type: 'L', x: 150, y: 200 },
        { type: 'L', x: 50, y: 200 },
        { type: 'Z', x: 50, y: 100 },
      ],
    ];
    const url = pathsToSvgDataUrl(figureEight);
    const svg = decodeURIComponent(url.slice('data:image/svg+xml;utf8,'.length));
    // 単一 <path> に nonzero が付き、evenodd は現れない = font と同じ塗り規則
    expect(svg).toMatch(/<path [^>]*fill-rule="nonzero"/);
    expect(svg).not.toContain('evenodd');
  });
});
