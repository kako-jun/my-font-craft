import type { Page } from '../App';

interface Props {
  currentPage: Page;
  onNavigate: (page: Page) => void;
}

export default function Header(props: Props) {
  return (
    <header class="header">
      <div class="header__inner">
        <button class="header__logo" onClick={() => props.onNavigate('home')}>
          MyFontCraft
        </button>
        <nav class="header__nav">
          <button
            class="header__link"
            classList={{ 'header__link--active': props.currentPage === 'template' }}
            onClick={() => props.onNavigate('template')}
          >
            テンプレート生成
          </button>
          <button
            class="header__link"
            classList={{ 'header__link--active': props.currentPage === 'upload' }}
            onClick={() => props.onNavigate('upload')}
          >
            フォント作成
          </button>
        </nav>
      </div>
    </header>
  );
}
