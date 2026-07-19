export default function About() {
  return (
    <div>
      <h1>このサイトについて</h1>

      <section class="page-section">
        <h2>MyFontCraftとは</h2>
        <div class="section__body section__body--wide">
          <p>MyFontCraftは、手書き文字からフォントファイルを作るためのブラウザツールです。</p>
          <p>紙に書く作業の手触りを残しつつ、生成処理は端末内で完結します。</p>
        </div>
      </section>

      <section class="page-section">
        <h2>対応文字</h2>
        <div class="section__body">
          <ul>
            <li>
              ひらがな <span class="num">83</span>字、カタカナ <span class="num">87</span>
              字に対応しています。
            </li>
            <li>
              英字、数字、よく使う記号の合計 <span class="num">102</span>字に対応しています。
            </li>
            <li>
              常用漢字 <span class="num">2,136</span>字に対応しています。
            </li>
          </ul>
        </div>
      </section>

      <section class="page-section">
        <h2>利用条件</h2>
        <div class="section__body">
          <ul>
            <li>インストール不要・登録不要・無料です。</li>
            <li>全処理がブラウザ内で完結し、端末の外に出ません。</li>
            <li>生成したフォントの権利は書いた人のものです。個人・商用とも自由です。</li>
          </ul>
        </div>
      </section>

      <section class="page-section">
        <h2>作者</h2>
        <div class="section__body media-block">
          <img
            src="https://github.com/kako-jun.png"
            alt="kako-jun"
            width="56"
            height="56"
            class="media-block__image"
          />
          <div>
            <p>kako-jun</p>
            <div class="action-row">
              <a class="act" href="https://llll-ll.com" target="_blank" rel="noopener noreferrer">
                作者サイト
              </a>
              <a
                class="act"
                href="https://github.com/kako-jun/my-font-craft"
                target="_blank"
                rel="noopener noreferrer"
              >
                GitHub
              </a>
            </div>
          </div>
        </div>
      </section>

      <section class="page-section">
        <h2>応援</h2>
        <div class="section__body action-row">
          <a
            class="act"
            href="https://github.com/sponsors/kako-jun"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub Sponsors
          </a>
          <a class="act" href="https://amzn.to/41dkZF1" target="_blank" rel="noopener noreferrer">
            Amazon
          </a>
        </div>
      </section>
    </div>
  );
}
