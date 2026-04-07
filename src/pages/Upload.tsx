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
import { IconFolder, IconZip, IconDownload, IconFont, IconUpload } from '../components/icons';

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

  // 未検出文字のリスト（imported は取得済み扱い）
  const missingChars = createMemo(() =>
    glyphStatuses()
      .filter((g) => g.status === 'empty')
      .map((g) => g.char),
  );

  function addMessage(msg: ProcessMessage) {
    setMessages((prev) => [...prev, msg]);
  }

  // Phase 1: スキャン（画像処理のみ。フォント生成はしない）
  // merge=true なら既存結果にマージ
  async function handleFiles(files: FileList | File[], merge = false) {
    const fileArray = Array.from(files);
    if (fileArray.length === 0) return;

    const prevResult = merge ? scanResult() : null;
    const prevStatuses = merge ? glyphStatuses() : [];

    setPhase('scanning');
    setMessages([]);
    setFontBlob(null);
    if (!merge) {
      setGlyphStatuses([]);
      setCorrectedPages([]);
      setScanResult(null);
    }

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
            const thumb = document.createElement('canvas');
            const scale = 300 / canvas.width;
            thumb.width = 300;
            thumb.height = Math.round(canvas.height * scale);
            thumb.getContext('2d')!.drawImage(canvas, 0, 0, thumb.width, thumb.height);
            setCorrectedPages((prev) => [
              ...prev,
              { pageIndex, dataUrl: thumb.toDataURL('image/png') },
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
          text: `追加スキャン完了（合計 ${acquired}/${total} 文字）`,
        });
      } else {
        // 新規スキャン
        setGlyphStatuses(newGlyphStatuses);
        setScanResult(result);
        const found = result.glyphs.length;
        const total = newGlyphStatuses.length;
        addMessage({
          type: 'info',
          text: `スキャン完了: ${found}/${total} 文字を取得しました。結果を確認してください。`,
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
    addMessage({ type: 'info', text: 'フォントを生成中...' });

    try {
      const fontBytes = await buildFont({
        familyName: props.fontName || 'MyHandwriting',
        glyphs: result.glyphs,
      });

      const blob = new Blob([fontBytes], { type: 'font/ttf' });
      setFontBlob(blob);
      addMessage({ type: 'success', text: 'フォント生成が完了しました!' });
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

  // 未検出文字のリトライ用テンプレートPDFをダウンロード
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

  // 追加スキャン用のファイルハンドラ（マージモード）
  function handleMergeFiles(files: FileList | File[]) {
    handleFiles(files, true);
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
          text: `フォントをインポートしました: ${imported} 文字をインポートとして追加`,
        });
      } else {
        // 新規: インポート結果をそのままセット
        setGlyphStatuses(result.statuses);
        setScanResult({ glyphs: result.glyphs });

        addMessage({
          type: 'success',
          text: `フォントをインポートしました: ${result.glyphs.length} 文字を読み込みました。`,
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
      // review 中はマージモード
      if (phase() === 'review') {
        handleMergeFiles(e.dataTransfer.files);
      } else {
        handleFiles(e.dataTransfer.files);
      }
    }
  }

  function handleFileInput(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    if (input.files) {
      if (phase() === 'review') {
        handleMergeFiles(input.files);
      } else {
        handleFiles(input.files);
      }
    }
  }

  function handleReset() {
    setPhase('idle');
    setMessages([]);
    setFontBlob(null);
    setGlyphStatuses([]);
    setCorrectedPages([]);
    setScanResult(null);
  }

  return (
    <div class="upload-page">
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
          <p>
            {phase() === 'review'
              ? '追加の画像をアップロード（既存の結果にマージします）'
              : 'スキャンした画像のフォルダまたはZIPをドラッグ&ドロップ'}
          </p>
          <p class="drop-zone__hint">JPEG, PNG, WebP に対応</p>
          <p>または</p>
          <div style="display:flex;gap:0.5rem;justify-content:center;flex-wrap:wrap">
            <button
              class="btn"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('folder-input')?.click();
              }}
            >
              <IconFolder /> フォルダを選択
            </button>
            <button
              class="btn"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('file-input')?.click();
              }}
            >
              <IconZip /> ZIPを選択
            </button>
            <button
              class="btn"
              onClick={(e) => {
                e.stopPropagation();
                document.getElementById('font-input')?.click();
              }}
            >
              <IconUpload /> 既存フォントを読み込む
            </button>
          </div>
          <input
            id="file-input"
            type="file"
            multiple
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
        </div>
      </Show>

      <Show when={phase() === 'idle'}>
        <div class="card upload-hint" style="margin-top:1rem">
          <h4>アップロードについて</h4>
          <ul class="upload-hint__list">
            <li>スキャンした画像をフォルダごと、またはZIPにまとめてアップロード</li>
            <li>ファイル名やフォルダ階層は自由（中に画像が含まれていればOK）</li>
            <li>ページの識別はテンプレートのQRコードで自動的に行います</li>
            <li>斜めに撮っても自動補正されます</li>
          </ul>
        </div>
      </Show>

      {/* スキャン中のプログレスバー */}
      <Show when={phase() === 'scanning'}>
        <div class="card" style="margin-top:1rem">
          <ProgressBar current={currentPage()} total={totalPages()} label="スキャン中..." />
        </div>
      </Show>

      {/* メッセージ */}
      <Show when={messages().length > 0}>
        <div class="messages" style="margin-top:1rem">
          <For each={messages()}>
            {(msg) => <div class={`message message--${msg.type}`}>{msg.text}</div>}
          </For>
        </div>
      </Show>

      {/* スキャン結果確認グリッド */}
      <Show when={glyphStatuses().length > 0}>
        <div class="card" style="margin-top:1rem">
          <ScanResultGrid glyphStatuses={glyphStatuses()} correctedPages={correctedPages()} />
        </div>
      </Show>

      {/* レビューフェーズ */}
      <Show when={phase() === 'review'}>
        <div class="card" style="margin-top:1rem;text-align:center">
          {/* 未検出文字がある場合のリトライ案内 */}
          <Show when={missingChars().length > 0}>
            <div style="margin-bottom:1rem;padding:1rem;background:#FFF3CD;border-radius:4px;text-align:left">
              <p style="color:#856404;font-weight:bold;margin-bottom:0.5rem">
                {missingChars().length} 文字が未検出です
              </p>
              <p style="color:#856404;font-size:0.9rem;margin-bottom:0.75rem">
                未検出の文字だけを集めたテンプレートを印刷し、書き直してスキャンすると追加できます。
                そのまま生成すると、未検出の文字は端末のフォントで代替表示されます。
              </p>
              <button class="btn" onClick={handleDownloadRetryTemplate}>
                未検出文字のテンプレートをダウンロード ({Math.ceil(missingChars().length / 30)}{' '}
                ページ)
              </button>
            </div>
          </Show>

          <p style="margin-bottom:1rem;color:var(--accent)">
            {missingChars().length === 0
              ? '全文字を取得しました! フォントを生成できます。'
              : '結果を確認して、このまま生成するか、追加スキャンしてください。'}
          </p>
          <div style="display:flex;gap:1rem;justify-content:center;flex-wrap:wrap">
            <button class="btn btn--primary" onClick={handleBuildFont}>
              <IconFont />{' '}
              {missingChars().length > 0
                ? `このまま生成する（${glyphStatuses().filter((g) => g.status !== 'empty').length} 文字）`
                : 'フォントを生成する'}
            </button>
            <button class="btn" onClick={handleReset}>
              やり直す
            </button>
          </div>
        </div>
      </Show>

      {/* ビルド中 */}
      <Show when={phase() === 'building'}>
        <div class="card" style="margin-top:1rem;text-align:center">
          <p style="color:var(--accent)">フォントを生成中...</p>
        </div>
      </Show>

      {/* 完了: ダウンロードボタン */}
      <Show when={phase() === 'done'}>
        <div class="card" style="margin-top:1rem;text-align:center">
          <h3>フォントが完成しました!</h3>
          <div style="display:flex;gap:1rem;justify-content:center;flex-wrap:wrap;margin-top:1rem">
            <button class="btn btn--primary" onClick={handleDownloadFont}>
              <IconDownload /> フォントをダウンロード (.ttf)
            </button>
            <button class="btn" onClick={handleReset}>
              最初からやり直す
            </button>
          </div>
        </div>
      </Show>
    </div>
  );
}
