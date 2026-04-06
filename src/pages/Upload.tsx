import { createSignal, Show, For } from 'solid-js';
import ProgressBar from '../components/ProgressBar';
import { processImages, type ProcessResult, type ProcessMessage } from '../lib/scanner/processor';
import { buildFont } from '../lib/font/builder';

interface Props {
  fontName: string;
}

export default function Upload(props: Props) {
  const [dragActive, setDragActive] = createSignal(false);
  const [processing, setProcessing] = createSignal(false);
  const [currentPage, setCurrentPage] = createSignal(0);
  const [totalPages, setTotalPages] = createSignal(0);
  const [messages, setMessages] = createSignal<ProcessMessage[]>([]);
  const [fontReady, setFontReady] = createSignal(false);
  const [fontBlob, setFontBlob] = createSignal<Blob | null>(null);

  function addMessage(msg: ProcessMessage) {
    setMessages((prev) => [...prev, msg]);
  }

  async function handleFiles(files: FileList | File[]) {
    const fileArray = Array.from(files);
    if (fileArray.length === 0) return;

    setProcessing(true);
    setMessages([]);
    setFontReady(false);
    setFontBlob(null);

    try {
      const result: ProcessResult = await processImages(fileArray, {
        onPageStart: (page, total) => {
          setCurrentPage(page);
          setTotalPages(total);
        },
        onMessage: addMessage,
      });

      addMessage({ type: 'info', text: `${result.glyphs.length} 文字を処理しました。フォントを生成中...` });

      const fontBytes = await buildFont({
        familyName: props.fontName || 'MyHandwriting',
        glyphs: result.glyphs,
      });

      const blob = new Blob([fontBytes], { type: 'font/ttf' });
      setFontBlob(blob);
      setFontReady(true);
      addMessage({ type: 'success', text: 'フォント生成が完了しました!' });
    } catch (err) {
      addMessage({ type: 'error', text: `処理に失敗しました: ${err instanceof Error ? err.message : String(err)}` });
    } finally {
      setProcessing(false);
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

  return (
    <div class="upload-page">
      <h2>2. フォントを作成する</h2>

      <div
        class="drop-zone"
        classList={{ 'drop-zone--active': dragActive() }}
        onDragOver={(e) => { e.preventDefault(); setDragActive(true); }}
        onDragLeave={() => setDragActive(false)}
        onDrop={handleDrop}
      >
        <p>ここにドラッグ&ドロップ</p>
        <p>または</p>
        <div style="display:flex;gap:0.5rem;justify-content:center;flex-wrap:wrap">
          <button class="btn" onClick={(e) => { e.stopPropagation(); document.getElementById('file-input')?.click(); }}>
            ZIPまたは画像を選択
          </button>
          <button class="btn" onClick={(e) => { e.stopPropagation(); document.getElementById('folder-input')?.click(); }}>
            フォルダを選択
          </button>
        </div>
        <input
          id="file-input"
          type="file"
          multiple
          accept="image/*,.zip"
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

      <Show when={processing()}>
        <div class="card" style="margin-top:1rem">
          <ProgressBar current={currentPage()} total={totalPages()} label="処理中..." />
        </div>
      </Show>

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

      <Show when={fontReady()}>
        <div class="card" style="margin-top:1rem;text-align:center">
          <h3>フォントが完成しました!</h3>
          <button class="btn btn--primary" onClick={handleDownloadFont} style="margin-top:1rem">
            フォントをダウンロード (.ttf)
          </button>
        </div>
      </Show>
    </div>
  );
}
