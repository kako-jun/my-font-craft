import { A } from '@solidjs/router';

export default function Home() {
  return (
    <div>
      <h1>手書きの字が、フォントになります。</h1>

      <section class="page-section">
        <h2>このサイトでできること</h2>
        <div class="section__body section__body--wide">
          <p>
            テンプレートに手書きして撮影すると、ブラウザ上で
            <span class="nowrap">TTFを作成できます。</span>
          </p>
        </div>
      </section>

      <section class="page-section">
        <h2>テンプレートを印刷する</h2>
        <div class="section__body">
          <p>まずテンプレートを印刷します。</p>
          <p>
            記入した用紙を撮影して読み込むと、<span class="nowrap">TTFを作成できます。</span>
          </p>
          <A class="act" href="/template">
            テンプレートを印刷する
          </A>
        </div>
      </section>

      <section class="page-section">
        <h2>撮影画像からフォントを作成する</h2>
        <div class="section__body">
          <p>すでに作ったフォントを読み込み、追加で撮影した文字を足せます。</p>
          <A class="act" href="/upload">
            撮影画像からフォントを作成する
          </A>
        </div>
      </section>

      <section class="page-section">
        <h2>利用条件</h2>
        <div class="section__body">
          <ul>
            <li>登録不要・無料です。</li>
            <li>画像は端末の外に出ません。</li>
            <li>生成したフォントは個人・商用とも自由に使えます。</li>
          </ul>
        </div>
      </section>
    </div>
  );
}
