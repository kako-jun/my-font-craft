import type { Page } from '../App';
import { IconPrinter, IconPen } from '../components/icons';

interface Props {
  onNavigate: (page: Page) => void;
}

export default function Home(props: Props) {
  return (
    <div class="home">
      <section class="hero card">
        <h1 class="hero__title">自分だけのフォントを作ろう</h1>
        <p class="hero__desc">
          テンプレートを印刷して手書き。スキャンするだけで、あなたの字がフォントになります。
        </p>
        <div class="hero__actions">
          <button class="btn btn--primary" onClick={() => props.onNavigate('template')}>
            <IconPrinter /> 1. テンプレートを印刷する
          </button>
          <button class="btn" onClick={() => props.onNavigate('upload')}>
            <IconPen /> 2. フォントを作成する
          </button>
        </div>
      </section>

      <section class="steps">
        <div class="step">
          <span class="step__number">1</span>
          <span>印刷</span>
        </div>
        <span class="step__arrow">→</span>
        <div class="step">
          <span class="step__number">2</span>
          <span>手書き</span>
        </div>
        <span class="step__arrow">→</span>
        <div class="step">
          <span class="step__number">3</span>
          <span>スキャン</span>
        </div>
        <span class="step__arrow">→</span>
        <div class="step">
          <span class="step__number">4</span>
          <span>完成!</span>
        </div>
      </section>

      <section class="features">
        <div class="card">
          <h3>プライバシー安心</h3>
          <p>すべての処理がブラウザ内で完結。画像がサーバーに送られることはありません。</p>
        </div>
        <div class="card">
          <h3>かんたん</h3>
          <p>テンプレートを印刷して手書き → スキャンするだけ。事前の画像加工は不要です。</p>
        </div>
        <div class="card">
          <h3>無料</h3>
          <p>ユーザー登録不要、完全無料。作成したフォントは個人・商用問わず自由に使えます。</p>
        </div>
      </section>
    </div>
  );
}
