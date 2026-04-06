import type { Page } from '../App';

interface Props {
  onNavigate: (page: Page) => void;
}

export default function Footer(props: Props) {
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
      </div>
      <p class="footer__sub">
        全処理がブラウザ内で完結。画像がサーバーに送られることはありません。
      </p>
    </footer>
  );
}
