import { createSignal, onMount, Show } from 'solid-js';
import type { Page } from '../App';
import { initWasm, getWasmBuildInfo } from '../lib/wasm/loader';

interface Props {
  onNavigate: (page: Page) => void;
}

export default function Footer(props: Props) {
  const [buildLabel, setBuildLabel] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      await initWasm();
      const info = getWasmBuildInfo();
      if (info) {
        const dt = new Date(Number(info.unixTs) * 1000);
        const ymd = dt.toISOString().slice(0, 10);
        setBuildLabel(`build ${info.sha} (${ymd})`);
      }
    } catch {
      // WASM ロード失敗時は表示しない
    }
  });

  return (
    <footer class="footer">
      <div class="footer__inner">
        <a
          href="https://llll-ll.com"
          target="_blank"
          rel="noopener noreferrer"
          class="footer__link"
        >
          llll-ll.com
        </a>
        <span class="footer__sep">|</span>
        <button class="footer__link footer__link--btn" onClick={() => props.onNavigate('about')}>
          About
        </button>
        <span class="footer__sep">|</span>
        <span class="footer__copy">&copy; kako-jun</span>
        <Show when={buildLabel()}>
          <span class="footer__sep">|</span>
          <span class="footer__build" title="WASM ビルド識別">
            {buildLabel()}
          </span>
        </Show>
      </div>
      <p class="footer__sub">
        全処理がブラウザ内で完結。画像がサーバーに送られることはありません。
      </p>
    </footer>
  );
}
