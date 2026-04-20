import { Show } from 'solid-js';
import { A } from '@solidjs/router';
import { wasmBuildInfo } from '../lib/wasm/loader';

export default function Footer() {
  // WASM はロードしない。他所で initWasm() が走ったらシグナルが立ち自動的に表示される
  const buildLabel = () => {
    const info = wasmBuildInfo();
    if (!info) return null;
    const ymd = new Date(Number(info.unixTs) * 1000).toISOString().slice(0, 10);
    return `build ${info.sha} (${ymd})`;
  };

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
        <A href="/about" class="footer__link">
          About
        </A>
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
