import { A } from '@solidjs/router';

export default function Header() {
  return (
    <header class="header">
      <div class="header__inner">
        <A href="/" class="header__logo">
          MyFontCraft
        </A>
      </div>
    </header>
  );
}
