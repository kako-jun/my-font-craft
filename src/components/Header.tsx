import { A, useLocation } from '@solidjs/router';

export default function Header() {
  const location = useLocation();

  return (
    <header class="header">
      <div class="header__inner">
        <A href="/" class="header__logo">
          MyFontCraft
        </A>
        <nav class="header__nav">
          <A
            href="/template"
            class="header__link"
            classList={{ 'header__link--active': location.pathname === '/template' }}
          >
            1. テンプレート
          </A>
          <A
            href="/upload"
            class="header__link"
            classList={{ 'header__link--active': location.pathname === '/upload' }}
          >
            2. フォント作成
          </A>
        </nav>
      </div>
    </header>
  );
}
