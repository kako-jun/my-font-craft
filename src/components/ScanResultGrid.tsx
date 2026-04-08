import { For, Show, createSignal, createMemo } from 'solid-js';
import type { GlyphStatus } from '../lib/scanner/processor';

interface Props {
  glyphStatuses: GlyphStatus[];
  correctedPages: { pageIndex: number; dataUrl: string }[];
  excludedChars?: Set<string>;
  onToggleExclude?: (char: string) => void;
}

export default function ScanResultGrid(props: Props) {
  const [modalImage, setModalImage] = createSignal<string | null>(null);
  const [modalTitle, setModalTitle] = createSignal('');

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

  const isExcluded = (char: string) => props.excludedChars?.has(char) ?? false;

  const stats = createMemo(() => {
    const total = props.glyphStatuses.length;
    const found = props.glyphStatuses.filter(
      (g) => g.status === 'found' && !isExcluded(g.char),
    ).length;
    const imported = props.glyphStatuses.filter(
      (g) => g.status === 'imported' && !isExcluded(g.char),
    ).length;
    const excluded = props.excludedChars?.size ?? 0;
    const acquired = found + imported;
    return {
      total,
      found,
      imported,
      excluded,
      acquired,
      pct: total > 0 ? Math.round((acquired / total) * 100) : 0,
    };
  });

  const pageThumb = (pageIndex: number) =>
    props.correctedPages.find((p) => p.pageIndex === pageIndex);

  function openModal(src: string, title: string) {
    setModalImage(src);
    setModalTitle(title);
  }

  function closeModal() {
    setModalImage(null);
    setModalTitle('');
  }

  return (
    <div class="scan-grid">
      {/* 拡大モーダル */}
      <Show when={modalImage()}>
        <div class="scan-grid__modal-overlay" onClick={closeModal}>
          <div class="scan-grid__modal" onClick={(e) => e.stopPropagation()}>
            <button class="scan-grid__modal-close" onClick={closeModal}>
              ×
            </button>
            <div class="scan-grid__modal-title">{modalTitle()}</div>
            <img class="scan-grid__modal-img" src={modalImage()!} alt={modalTitle()} />
          </div>
        </div>
      </Show>

      {/* サマリー */}
      <div class="scan-grid__summary">
        <span class="scan-grid__stat scan-grid__stat--found">取得: {stats().acquired}</span>
        <Show when={stats().imported > 0}>
          <span class="scan-grid__stat scan-grid__stat--imported">
            (スキャン: {stats().found} / インポート: {stats().imported})
          </span>
        </Show>
        <Show when={stats().excluded > 0}>
          <span class="scan-grid__stat scan-grid__stat--excluded">除外: {stats().excluded}</span>
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
                    onClick={() => openModal(thumb!.dataUrl, `Page ${pageIndex} — 補正後画像`)}
                    title="クリックで拡大"
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

              {/* 文字グリッド */}
              <div class="scan-grid__chars">
                <For each={glyphs}>
                  {(gs) => {
                    const excluded = () => isExcluded(gs.char);
                    const canToggle = () => gs.status !== 'empty' && props.onToggleExclude;
                    return (
                      <div
                        class="scan-grid__cell"
                        classList={{
                          'scan-grid__cell--found': gs.status === 'found' && !excluded(),
                          'scan-grid__cell--empty': gs.status === 'empty',
                          'scan-grid__cell--imported': gs.status === 'imported' && !excluded(),
                          'scan-grid__cell--excluded': excluded(),
                        }}
                        title={
                          excluded()
                            ? `${gs.char} — 除外中（クリックで復帰）`
                            : canToggle()
                              ? `${gs.char} (U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')}) — クリックで除外`
                              : `${gs.char} (U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')})`
                        }
                        onClick={(e) => {
                          if (canToggle()) {
                            // Shift+クリックで拡大、通常クリックで除外トグル
                            if (e.shiftKey && gs.cellImageDataUrl) {
                              openModal(
                                gs.cellImageDataUrl,
                                `${gs.char} (U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')})`,
                              );
                            } else {
                              props.onToggleExclude!(gs.char);
                            }
                          } else if (gs.cellImageDataUrl) {
                            openModal(
                              gs.cellImageDataUrl,
                              `${gs.char} (U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')})`,
                            );
                          }
                        }}
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
                        <Show when={excluded()}>
                          <div class="scan-grid__cell-excluded-mark">/</div>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
          );
        }}
      </For>
    </div>
  );
}
