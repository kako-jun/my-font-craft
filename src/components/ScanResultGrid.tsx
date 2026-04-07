import { For, Show, createMemo } from 'solid-js';
import type { GlyphStatus } from '../lib/scanner/processor';

interface Props {
  glyphStatuses: GlyphStatus[];
  correctedPages: { pageIndex: number; dataUrl: string }[];
}

export default function ScanResultGrid(props: Props) {
  // ページごとにグループ化
  const pageGroups = createMemo(() => {
    const groups = new Map<number, GlyphStatus[]>();
    for (const gs of props.glyphStatuses) {
      const list = groups.get(gs.pageIndex) ?? [];
      list.push(gs);
      groups.set(gs.pageIndex, list);
    }
    return Array.from(groups.entries()).sort((a, b) => a[0] - b[0]);
  });

  const stats = createMemo(() => {
    const total = props.glyphStatuses.length;
    const found = props.glyphStatuses.filter((g) => g.status === 'found').length;
    const imported = props.glyphStatuses.filter((g) => g.status === 'imported').length;
    const acquired = found + imported;
    return {
      total,
      found,
      imported,
      acquired,
      pct: total > 0 ? Math.round((acquired / total) * 100) : 0,
    };
  });

  const pageThumb = (pageIndex: number) =>
    props.correctedPages.find((p) => p.pageIndex === pageIndex);

  return (
    <div class="scan-grid">
      {/* サマリー */}
      <div class="scan-grid__summary">
        <span class="scan-grid__stat scan-grid__stat--found">取得: {stats().acquired}</span>
        <Show when={stats().imported > 0}>
          <span class="scan-grid__stat scan-grid__stat--imported">
            (スキャン: {stats().found} / インポート: {stats().imported})
          </span>
        </Show>
        <span class="scan-grid__stat scan-grid__stat--total">/ {stats().total} 文字</span>
        <span class="scan-grid__stat">({stats().pct}%)</span>
      </div>

      {/* ページごとのセクション */}
      <For each={pageGroups()}>
        {([pageIndex, glyphs]) => {
          const thumb = pageThumb(pageIndex);
          const pageFound = glyphs.filter((g) => g.status === 'found').length;
          const pageImported = glyphs.filter((g) => g.status === 'imported').length;
          const pageAcquired = pageFound + pageImported;
          return (
            <div class="scan-grid__page" id={`scan-page-${pageIndex}`}>
              {/* ページヘッダー: サムネイル + ステータス */}
              <div class="scan-grid__page-header">
                <Show when={thumb}>
                  <img
                    class="scan-grid__page-thumb"
                    src={thumb!.dataUrl}
                    alt={`Page ${pageIndex}`}
                  />
                </Show>
                <div class="scan-grid__page-info">
                  <h4>Page {pageIndex}</h4>
                  <span class="scan-grid__page-stat">
                    {pageAcquired}/{glyphs.length} 文字取得
                    {pageImported > 0 && ` (インポート: ${pageImported})`}
                  </span>
                </div>
              </div>

              {/* 文字グリッド（3列×10行 = テンプレートと同じ配置） */}
              <div class="scan-grid__chars">
                <For each={glyphs}>
                  {(gs) => (
                    <div
                      class="scan-grid__cell"
                      classList={{
                        'scan-grid__cell--found': gs.status === 'found',
                        'scan-grid__cell--empty': gs.status === 'empty',
                        'scan-grid__cell--imported': gs.status === 'imported',
                      }}
                      title={`${gs.char} (U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')}) — p${gs.pageIndex} r${gs.row} c${gs.col}`}
                    >
                      <div class="scan-grid__cell-char">{gs.char}</div>
                      <Show when={gs.cellImageDataUrl}>
                        <img
                          class="scan-grid__cell-img"
                          src={gs.cellImageDataUrl}
                          alt={gs.char}
                          loading="lazy"
                        />
                      </Show>
                      <Show when={gs.status === 'empty'}>
                        <div class="scan-grid__cell-miss">×</div>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </div>
          );
        }}
      </For>
    </div>
  );
}
