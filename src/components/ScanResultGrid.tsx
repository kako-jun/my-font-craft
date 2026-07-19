import { For, Show, createSignal, createMemo, createEffect, onMount, onCleanup } from 'solid-js';
import type { GlyphStatus } from '../lib/scanner/processor';

/**
 * 抽出結果の検分 UI（#114）。
 * 概念モデル: 夜の机に散らした紙片（俯瞰）→ 1枚ずつランプの光で検分 → 採用/書き直しに仕分け。
 * - 俯瞰: 全文字を枠なしの紙片として一覧。要確認（#110 needsReview）は琥珀色に明滅
 * - 検分: 紙片を選ぶと大きく表示。← → / スワイプで次へ
 * - 仕分け: Enter=採用（灯る）/ X=書き直し（リトライ対象）/ Space=次へ
 */

interface Props {
  glyphStatuses: GlyphStatus[];
  correctedPages: { pageIndex: number; dataUrl: string }[];
  /** 書き直し（リトライ対象 = フォントから除外） */
  excludedChars: Set<string>;
  /** 採用済み（灯り。生成対象なのは書き直し以外の全取得文字） */
  adoptedChars: Set<string>;
  onAdopt: (char: string) => void;
  onRewrite: (char: string) => void;
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

  // 検分の順路（俯瞰の並びと同一）
  const items = createMemo(() => pageGroups().flatMap(([, glyphs]) => glyphs));

  const isRewrite = (char: string) => props.excludedChars.has(char);
  const isAdopted = (char: string) => props.adoptedChars.has(char) && !isRewrite(char);
  const isReview = (gs: GlyphStatus) =>
    gs.status === 'found' && gs.needsReview === true && !isRewrite(gs.char);

  const stats = createMemo(() => {
    const total = props.glyphStatuses.length;
    const found = props.glyphStatuses.filter(
      (g) => g.status === 'found' && !isRewrite(g.char),
    ).length;
    const imported = props.glyphStatuses.filter(
      (g) => g.status === 'imported' && !isRewrite(g.char),
    ).length;
    const rewrite = props.excludedChars.size;
    const adopted = props.glyphStatuses.filter((g) => isAdopted(g.char)).length;
    const acquired = found + imported;
    return {
      total,
      found,
      imported,
      rewrite,
      adopted,
      acquired,
      pct: total > 0 ? Math.round((acquired / total) * 100) : 0,
    };
  });

  const pageThumb = (pageIndex: number) =>
    props.correctedPages.find((p) => p.pageIndex === pageIndex);

  // ---- 検分ビュー ----

  const [inspectIdx, setInspectIdx] = createSignal<number | null>(null);
  const current = createMemo(() => {
    const i = inspectIdx();
    return i === null ? null : (items()[i] ?? null);
  });

  // 検分を開いたらフォーカスをビューへ移す（俯瞰セルに残った Enter/Space の誤爆防止）
  let inspectorEl: HTMLDivElement | undefined;
  let wasOpen = false;
  createEffect(() => {
    const open = inspectIdx() !== null;
    if (open && !wasOpen) inspectorEl?.focus();
    wasOpen = open;
  });

  // 追加スキャンなどで並びが変わったら範囲外を閉じる
  createEffect(() => {
    const i = inspectIdx();
    if (i !== null && i >= items().length) setInspectIdx(null);
  });

  // 検分を開いた起点の紙片セル。閉じたらここへフォーカスを戻す（キーボード操作の迷子防止）
  let returnFocusEl: HTMLElement | null = null;
  // 検分ビュー内の可視フォーカス対象（disabled は除く）。Tab のトラップに使う
  const inspectorFocusables = (): HTMLElement[] =>
    inspectorEl
      ? Array.from(
          inspectorEl.querySelectorAll<HTMLElement>(
            'button:not([disabled]), [tabindex]:not([tabindex="-1"])',
          ),
        )
      : [];

  const openAt = (idx: number) => {
    const active = document.activeElement;
    returnFocusEl =
      active instanceof HTMLElement && active.classList.contains('scan-grid__cell') ? active : null;
    setInspectIdx(Math.max(0, Math.min(idx, items().length - 1)));
  };
  const close = () => {
    setInspectIdx(null);
    returnFocusEl?.focus();
    returnFocusEl = null;
  };
  const prev = () => setInspectIdx((i) => (i === null ? null : Math.max(0, i - 1)));
  const next = () =>
    setInspectIdx((i) => (i === null ? null : Math.min(items().length - 1, i + 1)));
  /** 次へ。最後の1枚なら検分を終える */
  const nextOrClose = () => {
    const i = inspectIdx();
    if (i === null) return;
    if (i >= items().length - 1) close();
    else setInspectIdx(i + 1);
  };

  const adoptCurrent = () => {
    const gs = current();
    if (!gs) return;
    if (gs.status !== 'empty') props.onAdopt(gs.char);
    nextOrClose();
  };

  const rewriteCurrent = () => {
    const gs = current();
    if (!gs) return;
    if (gs.status !== 'empty') props.onRewrite(gs.char);
    nextOrClose();
  };

  // 要確認の紙片から検分を始める（異常が先に目に入る）
  const firstInspectIdx = () => {
    const idx = items().findIndex((g) => isReview(g));
    return idx >= 0 ? idx : 0;
  };

  // ---- 補正後ページ画像のライトボックス ----

  const [lightbox, setLightbox] = createSignal<{ src: string; title: string } | null>(null);

  // ---- キーボード（マウス精密操作を要求しない） ----

  function handleKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA')) return;
    // 検分ビュー内でフォーカス中のボタンは native の活性化に任せる（二重発火防止）
    if (
      (e.key === 'Enter' || e.key === ' ') &&
      target instanceof HTMLElement &&
      target.tagName === 'BUTTON' &&
      target.closest('.inspector')
    ) {
      return;
    }

    if (lightbox()) {
      if (e.key === 'Escape') {
        e.preventDefault();
        setLightbox(null);
      }
      return;
    }

    if (inspectIdx() === null) {
      // 俯瞰から ← → で検分を開始
      if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
        e.preventDefault();
        openAt(firstInspectIdx());
      }
      return;
    }

    switch (e.key) {
      case 'ArrowRight':
        e.preventDefault();
        next();
        break;
      case 'ArrowLeft':
        e.preventDefault();
        prev();
        break;
      case 'Enter':
        e.preventDefault();
        adoptCurrent();
        break;
      case 'x':
      case 'X':
        e.preventDefault();
        rewriteCurrent();
        break;
      case ' ':
        e.preventDefault();
        nextOrClose();
        break;
      case 'Tab': {
        // フォーカストラップ: dialog 内の可視要素だけを巡回し、端で先頭/末尾へ折り返す
        const focusables = inspectorFocusables();
        if (focusables.length === 0) break;
        e.preventDefault();
        const cur = document.activeElement as HTMLElement | null;
        const at = cur ? focusables.indexOf(cur) : -1;
        const last = focusables.length - 1;
        let to: number;
        if (e.shiftKey) to = at <= 0 ? last : at - 1;
        else to = at === last || at === -1 ? 0 : at + 1;
        focusables[to].focus();
        break;
      }
      case 'Escape':
        e.preventDefault();
        close();
        break;
    }
  }

  onMount(() => window.addEventListener('keydown', handleKey));
  onCleanup(() => window.removeEventListener('keydown', handleKey));

  // スワイプ（検分中）
  let swipeStartX: number | null = null;
  const onStagePointerDown = (e: PointerEvent) => {
    swipeStartX = e.clientX;
  };
  const onStagePointerUp = (e: PointerEvent) => {
    if (swipeStartX === null) return;
    const dx = e.clientX - swipeStartX;
    swipeStartX = null;
    if (dx < -40) next();
    else if (dx > 40) prev();
  };

  const codeOf = (gs: GlyphStatus) => `U+${gs.unicode.toString(16).toUpperCase().padStart(4, '0')}`;

  const verdictLabel = (gs: GlyphStatus) => {
    if (gs.status === 'empty') return { label: '空', cls: '' };
    if (isRewrite(gs.char)) return { label: '書き直し', cls: 'inspector__verdict-state--rewrite' };
    if (isAdopted(gs.char)) return { label: '採用 — 灯', cls: 'inspector__verdict-state--adopted' };
    return { label: '未仕分け', cls: '' };
  };

  return (
    <div class="scan-grid">
      {/* ライトボックス（補正後ページ画像） */}
      <Show when={lightbox()}>
        <div class="lightbox" onClick={() => setLightbox(null)}>
          <button class="lightbox__close" onClick={() => setLightbox(null)}>
            ×
          </button>
          <img class="lightbox__img" src={lightbox()!.src} alt={lightbox()!.title} />
          <div class="lightbox__title">{lightbox()!.title}</div>
        </div>
      </Show>

      {/* 検分ビュー: ランプの光だまりの中の1枚 */}
      <Show when={current()}>
        {(gs) => (
          <div
            class="inspector"
            role="dialog"
            aria-modal="true"
            aria-label="紙片の検分"
            tabIndex={-1}
            ref={(el) => (inspectorEl = el)}
            /* 背景（stage 外）タップで閉じる — キーボード無しの脱出口 */
            onClick={(e) => {
              if (e.target === e.currentTarget) close();
            }}
          >
            <button class="inspector__close" onClick={close} title="閉じる (Esc)">
              ×
            </button>
            <div
              class="inspector__stage"
              onPointerDown={onStagePointerDown}
              onPointerUp={onStagePointerUp}
            >
              <button
                class="inspector__nav"
                onClick={prev}
                disabled={inspectIdx() === 0}
                title="前へ (←)"
              >
                ←
              </button>
              <figure
                class="inspector__slip"
                classList={{ 'inspector__slip--empty': !gs().cellImageDataUrl }}
              >
                <Show when={gs().cellImageDataUrl} fallback={<span>未検出</span>}>
                  <img src={gs().cellImageDataUrl} alt={gs().char} />
                </Show>
              </figure>
              <button
                class="inspector__nav"
                onClick={next}
                disabled={inspectIdx() === items().length - 1}
                title="次へ (→)"
              >
                →
              </button>
            </div>
            <div class="inspector__meta">
              <span class="inspector__char">{gs().char}</span>
              <span class="inspector__code">{codeOf(gs())}</span>
              <span class={`inspector__verdict-state ${verdictLabel(gs()).cls}`}>
                {verdictLabel(gs()).label}
              </span>
              <Show when={gs().needsReview === true}>
                <span class="inspector__flag">要確認 — ノイズを自動除去。字形を確認</span>
              </Show>
            </div>
            <div class="inspector__verdicts">
              <button
                class="inspector__verdict inspector__verdict--adopt"
                onClick={adoptCurrent}
                disabled={gs().status === 'empty'}
              >
                採用<kbd>Enter</kbd>
              </button>
              <button
                class="inspector__verdict inspector__verdict--rewrite"
                onClick={rewriteCurrent}
                disabled={gs().status === 'empty'}
              >
                書き直し<kbd>X</kbd>
              </button>
              <button class="inspector__verdict inspector__verdict--skip" onClick={nextOrClose}>
                次へ<kbd>Space</kbd>
              </button>
            </div>
            <div class="inspector__pos">
              {inspectIdx()! + 1} / {items().length}
            </div>
          </div>
        )}
      </Show>

      {/* サマリー */}
      <div class="scan-grid__summary">
        <span class="scan-grid__stat scan-grid__stat--found">
          取得 <span class="num">{stats().acquired}</span>/<span class="num">{stats().total}</span>{' '}
          字（<span class="num">{stats().pct}%</span>）
        </span>
        <Show when={stats().imported > 0}>
          <span class="scan-grid__stat">
            うちインポート <span class="num">{stats().imported}</span>
          </span>
        </Show>
        <Show when={stats().adopted > 0}>
          <span class="scan-grid__stat scan-grid__stat--found">
            採用 <span class="num">{stats().adopted}</span>
          </span>
        </Show>
        <Show when={stats().rewrite > 0}>
          <span class="scan-grid__stat scan-grid__stat--excluded">
            書き直し <span class="num">{stats().rewrite}</span>
          </span>
        </Show>
      </div>

      {/* 操作ヒント */}
      <div class="scan-grid__hint">
        紙片を選んで検分 — <kbd>←</kbd> <kbd>→</kbd> 移動 / <kbd>Enter</kbd> 採用 / <kbd>X</kbd>{' '}
        書き直し / <kbd>Space</kbd> 次へ
      </div>

      {/* ページごとの紙片 */}
      <For each={pageGroups()}>
        {([pageIndex, glyphs]) => {
          const thumb = pageThumb(pageIndex);
          const pageAcquired = glyphs.filter((g) => g.status !== 'empty').length;
          return (
            <div class="scan-grid__page" id={`scan-page-${pageIndex}`}>
              <div class="scan-grid__page-header">
                <Show when={thumb}>
                  <img
                    class="scan-grid__page-thumb"
                    src={thumb!.dataUrl}
                    alt={`Page ${pageIndex}`}
                    onClick={() =>
                      setLightbox({ src: thumb!.dataUrl, title: `Page ${pageIndex} — 補正後画像` })
                    }
                    title="クリックで拡大"
                  />
                </Show>
                <div class="scan-grid__page-info">
                  <h4>Page {pageIndex}</h4>
                  <span class="scan-grid__page-stat">
                    <span class="num">{pageAcquired}</span>/<span class="num">{glyphs.length}</span>{' '}
                    字
                  </span>
                </div>
              </div>

              <div class="scan-grid__chars">
                <For each={glyphs}>
                  {(gs) => {
                    const rewrite = () => isRewrite(gs.char);
                    const adopted = () => isAdopted(gs.char);
                    const review = () => isReview(gs);
                    const title = () => `${gs.char} (${codeOf(gs)}) — クリックで検分`;
                    const open = () => openAt(items().indexOf(gs));
                    return (
                      <div
                        class="scan-grid__cell"
                        role="button"
                        tabIndex={0}
                        classList={{
                          'scan-grid__cell--found': gs.status === 'found' && !rewrite(),
                          'scan-grid__cell--empty': gs.status === 'empty',
                          'scan-grid__cell--imported': gs.status === 'imported' && !rewrite(),
                          'scan-grid__cell--excluded': rewrite(),
                          'scan-grid__cell--review': review(),
                          'scan-grid__cell--adopted': adopted(),
                        }}
                        title={title()}
                        onClick={open}
                        onKeyDown={(e) => {
                          if (e.key !== 'Enter' && e.key !== ' ') return;
                          e.preventDefault();
                          e.stopPropagation();
                          open();
                        }}
                      >
                        <Show
                          when={gs.cellImageDataUrl}
                          fallback={<div class="scan-grid__cell-ghost">{gs.char}</div>}
                        >
                          <img
                            class="scan-grid__cell-img"
                            src={gs.cellImageDataUrl}
                            alt={gs.char}
                            loading="lazy"
                          />
                        </Show>
                        <div class="scan-grid__cell-char">{gs.char}</div>
                        <Show when={rewrite()}>
                          <div class="scan-grid__cell-excluded-mark">X</div>
                        </Show>
                        {/* 品質ゲート（#110）: ノイズ自動除去セル。黙って空に倒さず目視確認を促す */}
                        <Show when={review()}>
                          <div
                            class="scan-grid__cell-review-mark"
                            title="要確認 — 検分で字形を確認"
                          >
                            !
                          </div>
                        </Show>
                        <Show when={adopted()}>
                          <div class="scan-grid__cell-lit" />
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
