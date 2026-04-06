import { createSignal, Show, For } from 'solid-js';
import ProgressBar from '../components/ProgressBar';
import ScanResultGrid from '../components/ScanResultGrid';
import { processImages, type ProcessResult, type ProcessMessage, type GlyphStatus } from '../lib/scanner/processor';
import { buildFont, type VectorGlyph } from '../lib/font/builder';

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
  const [correctedPages, setCorrectedPages] = createSignal<{ pageIndex: number; dataUrl: string }[]>([]);
  const [scanResult, setScanResult] = createSignal<ProcessResult | null>(null);

  function addMessage(msg: ProcessMessage) {
    setMessages((prev) => [...prev, msg]);
  }

  // Phase 1: スキャン（画像処理のみ。フォント生成はしない）
  async function handleFiles(files: FileList | File[]) {
    const fileArray = Array.from(files);
    if (fileArray.length === 0) return;

    setPhase('scanning');
    setMessages([]);
    setFontBlob(null);
    setGlyphStatuses([]);
    setCorrectedPages([]);
    setScanResult(null);

    try {
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
            setCorrectedPages(prev => [...prev, { pageIndex, dataUrl: thumb.toDataURL('image/png') }]);
          } catch { /* ignore */ }
        },
        onGlyphStatus: (status) => {
          setGlyphStatuses(prev => [...prev, status]);
        },
      });

      setScanResult(result);
      const found = result.glyphs.length;
      const total = glyphStatuses().length;
      addMessage({
        type: 'info',
        text: `スキャン完了: ${found}/${total} 文字を取得しました。結果を確認してください。`,
      });
      setPhase('review');
    } catch (err) {
      addMessage({ type: 'error', text: `処理に失敗しました: ${err instanceof Error ? err.message : String(err)}` });
      setPhase('idle');
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
      addMessage({ type: 'error', text: `フォント生成に失敗しました: ${err instanceof Error ? err.message : String(err)}` });
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
    }
  }

  // やり直し
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
          onDragOver={(e) => { e.preventDefault(); setDragActive(true); }}
          onDragLeave={() => setDragActive(false)}
          onDrop={handleDrop}
        >
          <p>
            {phase() === 'review'
              ? '別の画像を追加でアップロード（現在の結果に上書き）'
              : 'スキャンした画像のフォルダまたはZIPをドラッグ&ドロップ'}
          </p>
          <p class="drop-zone__hint">JPEG, PNG, WebP に対応</p>
          <p>または</p>
          <div style="display:flex;gap:0.5rem;justify-content:center;flex-wrap:wrap">
            <button class="btn" onClick={(e) => { e.stopPropagation(); document.getElementById('folder-input')?.click(); }}>
              フォルダを選択
            </button>
            <button class="btn" onClick={(e) => { e.stopPropagation(); document.getElementById('file-input')?.click(); }}>
              ZIPを選択
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
            // @ts-ignore webkitdirectory
            webkitdirectory
            style="display:none"
            onChange={handleFileInput}
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
            {(msg) => (
              <div class={`message message--${msg.type}`}>
                {msg.text}
              </div>
            )}
          </For>
        </div>
      </Show>

      {/* スキャン結果確認グリッド */}
      <Show when={glyphStatuses().length > 0 && phase() !== 'scanning'}>
        <div class="card" style="margin-top:1rem">
          <ScanResultGrid
            glyphStatuses={glyphStatuses()}
            correctedPages={correctedPages()}
          />
        </div>
      </Show>

      {/* レビューフェーズ: フォント生成ボタン */}
      <Show when={phase() === 'review'}>
        <div class="card" style="margin-top:1rem;text-align:center">
          <p style="margin-bottom:1rem;color:var(--accent)">
            結果を確認して、問題なければフォントを生成してください。
          </p>
          <div style="display:flex;gap:1rem;justify-content:center;flex-wrap:wrap">
            <button class="btn btn--primary" onClick={handleBuildFont}>
              フォントを生成する
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
              フォントをダウンロード (.ttf)
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
