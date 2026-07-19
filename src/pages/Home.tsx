import { A } from '@solidjs/router';

export default function Home() {
  return (
    <div class="home">
      <div class="home__lead">
        <h1>
          手書きの字が、
          <br />
          フォントになる。
        </h1>
      </div>

      <div class="home__columns">
        {/* 4ステップ: 読まずに流れが分かる。1と3がそのまま入口 */}
        <ol class="flow home__flow">
          <li class="flow__step">
            <span class="flow__num">1</span>
            <div>
              <span class="flow__label">
                <A href="/template">テンプレートをダウンロード</A>
              </span>
              <span class="flow__hint">A4のPDF。印刷して使う</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">2</span>
            <div>
              <span class="flow__label">マスに手書き</span>
              <span class="flow__hint">ペンで一文字ずつ</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">3</span>
            <div>
              <span class="flow__label">
                <A href="/upload">撮影してアップロード</A>
              </span>
              <span class="flow__hint">スマホ撮影でも可。傾きは自動補正</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">4</span>
            <div>
              <span class="flow__label">フォントをダウンロード</span>
              <span class="flow__hint">
                <span class="num">.ttf</span> — そのままPCやスマホで使える
              </span>
            </div>
          </li>
        </ol>

        {/* 操作サンプル: 記入済みテンプレートの紙片 */}
        <figure class="home__samples">
          <img
            class="sample-slip sample-slip--page"
            src="/sample-template-page.webp"
            alt="記入済みテンプレートの全体"
            width="560"
            height="792"
          />
          <img
            class="sample-slip sample-slip--cells"
            src="/sample-filled-cells.webp"
            alt="マスに手書きした文字の拡大"
            width="720"
            height="351"
          />
          <figcaption>記入例 — マスに書いた字がそのまま字形になる</figcaption>
        </figure>
      </div>

      <ul class="facts">
        <li>登録不要・無料</li>
        <li>生成したフォントは個人・商用とも自由</li>
      </ul>
    </div>
  );
}
