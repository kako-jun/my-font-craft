import { createSignal, createMemo, Show, For } from 'solid-js';
import ProgressBar from '../components/ProgressBar';
import ScanResultGrid from '../components/ScanResultGrid';
import {
  processImages,
  type ProcessResult,
  type ProcessMessage,
  type GlyphStatus,
} from '../lib/scanner/processor';
import { buildFont, importFont } from '../lib/font/builder';
import { mergeScanIntoExisting, mergeImportIntoExisting } from '../lib/merge';
import { generateRetryTemplatePDF } from '../lib/template/generator';

interface Props {
  fontName: string;
}

type Phase = 'idle' | 'scanning' | 'review' | 'building' | 'done';

export default function Upload(props: Props) {
  const [dragActive, setDragActive] = createSignal(false);
  const [phase, setPhase] = createSignal<Phase>('idle');
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(0);
  const [messages, setMessages] = createSignal<ProcessMessage[]>([]);
  const [fontBlob, setFontBlob] = createSignal<Blob | null>(null);
  const [glyphStatuses, setGlyphStatuses] = createSignal<GlyphStatus[]>([]);
  const [correctedPages, setCorrectedPages] = createSignal<
    { pageIndex: number; dataUrl: string }[]
  >([]);
  const [scanResult, setScanResult] = createSignal<ProcessResult | null>(null);
  // 仕分け（#114）: 書き直し = リトライ対象（フォントから除外）/ 採用 = 灯り
  const [excludedChars, setExcludedChars] = createSignal<Set<string>>(new Set());
  const [adoptedChars, setAdoptedChars] = createSignal<Set<string>>(new Set());

  // 未検出文字のリスト（imported は取得済み扱い、書き直しは未検出扱い = リトライ対象）
  const missingChars = createMemo(() =>
    glyphStatuses()
      .filter((g) => g.status === 'empty' || excludedChars().has(g.char))
      .map((g) => g.char),
  );

  /** 採用: 書き直しを取り消して灯す */
  function adoptChar(char: string) {
    setExcludedChars((prev) => {
      if (!prev.has(char)) return prev;
      const next = new Set(prev);
      next.delete(char);
      return next;
    });
    setAdoptedChars((prev) => {
      if (prev.has(char)) return prev;
      const next = new Set(prev);
      next.add(char);
      return next;
    });
  }

  /** 書き直し: リトライ対象へ（フォントからは除外） */
  function rewriteChar(char: string) {
    setAdoptedChars((prev) => {
      if (!prev.has(char)) return prev;
      const next = new Set(prev);
      next.delete(char);
      return next;
    });
    setExcludedChars((prev) => {
      if (prev.has(char)) return prev;
      const next = new Set(prev);
      next.add(char);
      return next;
    });
  }

  // 取得済み文字（書き直しを除く）が1件以上あるか
  const hasAcquiredChars = createMemo(() =>
    glyphStatuses().some(
      (g) => (g.status === 'found' || g.status === 'imported') && !excludedChars().has(g.char),
    ),
  );

  // 生成対象の文字数（空・書き直し以外）
  const buildCount = createMemo(
    () =>
      glyphStatuses().filter((g) => g.status !== 'empty' && !excludedChars().has(g.char)).length,
  );

  function addMessage(msg: ProcessMessage) {
    setMessages((prev) => [...prev, msg]);
  }

  // Phase 1: スキャン（画像処理のみ。フォント生成はしない）
  // 既存結果がある場合は常にマージ（追加）。リセットしたい場合は handleReset を明示的に呼ぶ
  async function handleFiles(files: FileList | File[]) {
    const fileArray = Array.from(files);
    if (fileArray.length === 0) return;

    const existingResult = scanResult();
    const existingStatuses = glyphStatuses();
    const merge = existingResult !== null && existingStatuses.length > 0;
    const prevResult = merge ? existingResult : null;
    const prevStatuses = merge ? existingStatuses : [];

    setPhase('scanning');
    setMessages([]);
    setFontBlob(null);

    try {
      const newGlyphStatuses: GlyphStatus[] = [];
      const result: ProcessResult = await processImages(fileArray, {
        onPageStart: (page, total) => {
          setCurrentPage(page);
          setTotalPages(total);
        },
        onMessage: addMessage,
        onPageCorrected: (pageIndex, canvas) => {
          try {
            // 拡大表示に耐えるよう800px幅で保存
            const thumb = document.createElement('canvas');
            const scale = 800 / canvas.width;
            thumb.width = 800;
            thumb.height = Math.round(canvas.height * scale);
            thumb.getContext('2d')!.drawImage(canvas, 0, 0, thumb.width, thumb.height);
            setCorrectedPages((prev) => [
              ...prev,
              { pageIndex, dataUrl: thumb.toDataURL('image/jpeg', 0.85) },
            ]);
          } catch {
            /* ignore */
          }
        },
        onGlyphStatus: (status) => {
          newGlyphStatuses.push(status);
          // リアルタイムにグリッドを更新
          if (!merge) {
            setGlyphStatuses((prev) => [...prev, status]);
          }
        },
      });

      if (merge && prevResult) {
        // マージ: 新しく取得できた文字で既存の empty / imported を上書き（スキャンが優先）
        const merged = mergeScanIntoExisting(
          prevStatuses,
          prevResult.glyphs,
          newGlyphStatuses,
          result.glyphs,
        );
        setGlyphStatuses(merged.statuses);
        setScanResult({ glyphs: merged.glyphs });

        const total = merged.statuses.length;
        const found = merged.statuses.filter((g) => g.status === 'found').length;
        const imported = merged.statuses.filter((g) => g.status === 'imported').length;
        const acquired = found + imported;
        addMessage({
          type: 'info',
          text: `追加スキャン完了 — 合計 ${acquired}/${total} 字`,
        });
      } else {
        // 新規スキャン
        setGlyphStatuses(newGlyphStatuses);
        setScanResult(result);
        const found = result.glyphs.length;
        const total = newGlyphStatuses.length;
        addMessage({
          type: 'info',
          text: `スキャン完了 ${found}/${total} 字`,
        });
      }
      setPhase('review');
    } catch (err) {
      addMessage({
        type: 'error',
        text: `処理に失敗しました: ${err instanceof Error ? err.message : String(err)}`,
      });
      setPhase(prevResult ? 'review' : 'idle');
    }
  }

  // Phase 2: フォント生成（ユーザーがボタンを押してから）
  async function handleBuildFont() {
    const result = scanResult();
    if (!result) return;

    setPhase('building');

    try {
      // 書き直し文字のグリフを除く
      const excluded = excludedChars();
      const glyphs =
        excluded.size > 0
          ? result.glyphs.filter((g) => {
              if (g.unicode === undefined) return true;
              const char = String.fromCodePoint(g.unicode);
              return !excluded.has(char);
            })
          : result.glyphs;

      const fontBytes = await buildFont({
        familyName: props.fontName || 'MyHandwriting',
        glyphs,
      });

      const blob = new Blob([fontBytes], { type: 'font/ttf' });
      setFontBlob(blob);
      setPhase('done');
    } catch (err) {
      addMessage({
        type: 'error',
        text: `フォント生成に失敗しました: ${err instanceof Error ? err.message : String(err)}`,
      });
      setPhase('review');
    }
  }

  function handleDownloadFont() {
    const blob = fontBlob();
    if (!blob) return;
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${props.fontName || 'MyHandwriting'}.ttf`;
    a.click();
    URL.revokeObjectURL(url);
  }

  // 書き直し・未検出文字のリトライ用テンプレートPDFをダウンロード
  async function handleDownloadRetryTemplate() {
    const chars = missingChars();
    if (chars.length === 0) return;

    try {
      const pdfBytes = await generateRetryTemplatePDF(chars, props.fontName || 'MyHandwriting');
      const blob = new Blob([pdfBytes], { type: 'application/pdf' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `MyFontCraft-retry-${chars.length}chars.pdf`;
      a.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      addMessage({
        type: 'error',
        text: `テンプレート生成に失敗しました: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }

  // 既存TTF/OTFインポート
  async function handleImportFont(file: File) {
    try {
      const buffer = await file.arrayBuffer();
      const result = importFont(buffer);

      if (result.glyphs.length === 0) {
        addMessage({ type: 'warning', text: 'フォントにグリフが含まれていませんでした。' });
        return;
      }

      const prevStatuses = glyphStatuses();
      const prevResult = scanResult();

      if (prevStatuses.length > 0 && prevResult) {
        // マージ: empty のみを imported で埋める（found は上書きしない）
        const merged = mergeImportIntoExisting(
          prevStatuses,
          prevResult.glyphs,
          result.statuses,
          result.glyphs,
        );
        setGlyphStatuses(merged.statuses);
        setScanResult({ glyphs: merged.glyphs });

        const imported = merged.statuses.filter((g) => g.status === 'imported').length;
        addMessage({
          type: 'success',
          text: `インポート完了 — ${imported} 字を追加`,
        });
      } else {
        // 新規: インポート結果をそのままセット
        setGlyphStatuses(result.statuses);
        setScanResult({ glyphs: result.glyphs });

        addMessage({
          type: 'success',
          text: `インポート完了 ${result.glyphs.length} 字`,
        });
      }

      setPhase('review');
    } catch (err) {
      addMessage({
        type: 'error',
        text: `フォントの読み込みに失敗しました: ${err instanceof Error ? err.message : String(err)}`,
      });
    }
  }

  function handleFontFileInput(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files && input.files.length > 0) {
      handleImportFont(input.files[0]);
      input.value = ''; // 同じファイルを再選択可能にする
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    setDragActive(false);
    if (e.dataTransfer?.files) {
      handleFiles(e.dataTransfer.files);
    }
  }

  function handleFileInput(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files) {
      handleFiles(input.files);
      input.value = ''; // 同じファイルを再選択可能にする
    }
  }

  function handleReset() {
    setPhase('idle');
    setMessages([]);
    setFontBlob(null);
    setGlyphStatuses([]);
    setCorrectedPages([]);
    setScanResult(null);
    setExcludedChars(new Set<string>());
    setAdoptedChars(new Set<string>());
  }

  function handleResetWithConfirm() {
    if (
      typeof window !== 'undefined' &&
      !window.confirm('読み込んだすべての文字を破棄して 0 文字に戻します。よろしいですか？')
    ) {
      return;
    }
    handleReset();
  }

  return (
    <div class="upload-page" classList={{ 'upload-page--review': phase() === 'review' }}>
      <h2>2. フォントを作成する</h2>

      {/* ドロップゾーン（idle / review 時に表示） */}
      <Show when={phase() === 'idle' || phase() === 'review'}>
        <div
          class="drop-zone"
          classList={{ 'drop-zone--active': dragActive() }}
          onDragOver={(e) => {
            e.preventDefault();
            setDragActive(true);
          }}
          onDragLeave={() => setDragActive(false)}
          onDrop={handleDrop}
        >
          <p class="drop-zone__lead">
            {phase() === 'review'
              ? '追加の画像をここへ（既存に追加）'
              : 'スキャン画像・フォルダ・ZIPをここへ'}
          </p>
          <p class="drop-zone__hint">JPEG / PNG / WebP</p>
          <div class="drop-zone__sources">
            <button
              class="act"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('image-input')?.click();
              }}
            >
              画像を選択（複数可）
            </button>
            <button
              class="act"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('folder-input')?.click();
              }}
            >
              フォルダを選択
            </button>
            <button
              class="act"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('zip-input')?.click();
              }}
            >
              ZIPを選択
            </button>
            <button
              class="act"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('font-input')?.click();
              }}
            >
              既存フォントを読み込む
            </button>
          </div>
          <input
            id="image-input"
            type="file"
            multiple
            accept="image/*"
            style="display:none"
            onChange={handleFileInput}
          />
          <input
            id="zip-input"
            type="file"
            accept=".zip"
            style="display:none"
            onChange={handleFileInput}
          />
          <input
            id="folder-input"
            type="file"
            // @ts-expect-error webkitdirectory is a non-standard attribute
            webkitdirectory
            style="display:none"
            onChange={handleFileInput}
          />
          <input
            id="font-input"
            type="file"
            accept=".ttf,.otf"
            style="display:none"
            onChange={handleFontFileInput}
          />
          <Show when={phase() === 'review'}>
            <div class="drop-zone__reset">
              <button
                class="act act--quiet"
                onClick={(e) => {
                  e.stopPropagation();
                  handleResetWithConfirm();
                }}
              >
                リセット（0 文字に戻す）
              </button>
            </div>
          </Show>
        </div>
      </Show>

      <Show when={phase() === 'idle'}>
        <ul class="upload-hint__list upload-hint">
          <li>フォルダごと / ZIPでまとめて可。階層・ファイル名は自由</li>
          <li>ページ識別はQRコードで自動。多少の傾きも自動補正</li>
          <li>チェック欄は任意 — 同じ文字を2マス書いたとき ✓ を優先</li>
          <li>読み込みは常に既存へ追加。0 に戻すのは「リセット」</li>
        </ul>
      </Show>

      {/* 撮影ガイド: idle時と、エラー発生時（review/idle問わず）に表示 */}
      <Show when={phase() === 'idle' || messages().some((m) => m.type === 'error')}>
        <div class="shooting-guide">
          <h4 class="shooting-guide__title">撮影のコツ</h4>
          <div class="shooting-guide__item">
            <span class="shooting-guide__icon shooting-guide__icon--good">&#x2713;</span>
            <span>正面から、紙全体を収める</span>
          </div>
          <div class="shooting-guide__item">
            <span class="shooting-guide__icon shooting-guide__icon--bad">&#x2717;</span>
            <span>斜めすぎ — マーカーを検出できない</span>
          </div>
          <div class="shooting-guide__item">
            <span class="shooting-guide__icon shooting-guide__icon--bad">&#x2717;</span>
            <span>近すぎ・遠すぎ — 見切れ / ぼやけで読み取れない</span>
          </div>
        </div>
      </Show>

      {/* スキャン中のプログレスバー */}
      <Show when={phase() === 'scanning'}>
        <ProgressBar current={currentPage()} total={totalPages()} label="スキャン中..." />
      </Show>

      {/* メッセージ */}
      <Show when={messages().length > 0}>
        <div class="messages" style="margin-top:1.25rem">
          <For each={messages()}>
            {(msg) => <div class={`message message--${msg.type}`}>{msg.text}</div>}
          </For>
        </div>
      </Show>

      {/* 俯瞰: 抽出した紙片の一覧（検分・仕分けは ScanResultGrid 内） */}
      <Show when={glyphStatuses().length > 0}>
        <ScanResultGrid
          glyphStatuses={glyphStatuses()}
          correctedPages={correctedPages()}
          excludedChars={excludedChars()}
          adoptedChars={adoptedChars()}
          onAdopt={adoptChar}
          onRewrite={rewriteChar}
        />
      </Show>

      {/* 出口バー: 仕分けの出口が常に見える（review 中固定表示） */}
      <Show when={phase() === 'review'}>
        <div class="exit-bar">
          <div class="exit-bar__inner">
            <Show when={missingChars().length > 0}>
              <span>
                {/* リトライPDFは v:3 QR の `s` フラグに未対応のため無効化中（Issue #96） */}
                <button
                  class="act act--ember"
                  onClick={handleDownloadRetryTemplate}
                  disabled
                  title="調整中（Issue #96）"
                >
                  書き直し {missingChars().length} 字 → リトライPDF
                </button>{' '}
                <span class="exit-bar__note">調整中（#96）</span>
              </span>
            </Show>
            <button
              class="act act--primary"
              onClick={handleBuildFont}
              disabled={!hasAcquiredChars()}
            >
              {!hasAcquiredChars()
                ? 'フォントを生成できません'
                : missingChars().length > 0
                  ? `このまま生成する（${buildCount()} 文字）`
                  : 'フォントを生成する'}
            </button>
          </div>
        </div>
      </Show>

      {/* ビルド中 */}
      <Show when={phase() === 'building'}>
        <p class="upload-status">フォントを生成中...</p>
      </Show>

      {/* 完了 */}
      <Show when={phase() === 'done'}>
        <div class="upload-done">
          <h3>フォントが完成しました</h3>
          <div class="upload-done__actions">
            <button class="act act--primary" onClick={handleDownloadFont}>
              フォントをダウンロード (.ttf)
            </button>
            <button class="act act--quiet" onClick={handleResetWithConfirm}>
              最初からやり直す
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
}
