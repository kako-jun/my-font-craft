import { A, useLocation, useNavigate } from '@solidjs/router';

export default function Header() {
  const location = useLocation();
  const navigate = useNavigate();

  return (
    <header class="header">
      <div class="header__inner">
        <button type="button" class="header__logo" onClick={() => navigate('/')}>
          MyFontCraft
        </button>
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
