export default function About() {
  return (
    <div class="about-page">
      <section>
        <h2>MyFontCraft</h2>
        <p>手書きの字が、フォントになる。テンプレートに書いて、撮影して、TTFを受け取る。</p>
      </section>

      <section>
        <h3>流れ</h3>
        <ol class="flow">
          <li class="flow__step">
            <span class="flow__num">1</span>
            <div>
              <span class="flow__label">テンプレートをダウンロード・印刷</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">2</span>
            <div>
              <span class="flow__label">マスに手書き</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">3</span>
            <div>
              <span class="flow__label">撮影してアップロード</span>
              <span class="flow__hint">ページ識別はQRコード。傾きは自動補正</span>
            </div>
          </li>
          <li class="flow__step">
            <span class="flow__num">4</span>
            <div>
              <span class="flow__label">
                フォントをダウンロード（<span class="num">.ttf</span>）
              </span>
            </div>
          </li>
        </ol>
      </section>

      <section>
        <h3>対応文字</h3>
        <ul class="about-list">
          <li>
            ひらがな <span class="num">83</span>字 —{' '}
            <span class="about-chars__sample">あいうえお…</span>
          </li>
          <li>
            カタカナ <span class="num">87</span>字 —{' '}
            <span class="about-chars__sample">アイウエオ…</span>
          </li>
          <li>
            英数字・記号 <span class="num">102</span>字 —{' '}
            <span class="about-chars__sample">ABC abc 123 !?@#…</span>
          </li>
          <li>
            常用漢字 <span class="num">2,136</span>字 —{' '}
            <span class="about-chars__sample">亜哀挨愛曖…</span>
          </li>
        </ul>
      </section>

      <section>
        <h3>事実</h3>
        <ul class="about-list">
          <li>インストール不要・登録不要・無料</li>
          <li>生成したフォントの権利は書いた人のもの。個人・商用とも自由</li>
        </ul>
      </section>

      <section>
        <h3>作者</h3>
        <div class="about-author">
          <img
            src="https://github.com/kako-jun.png"
            alt="kako-jun"
            width="56"
            height="56"
            class="about-author__avatar"
          />
          <div>
            <p>kako-jun</p>
            <div class="about-links">
              <a
                class="act act--quiet"
                href="https://llll-ll.com"
                target="_blank"
                rel="noopener noreferrer"
              >
                llll-ll.com
              </a>
              <a
                class="act act--quiet"
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

      <section>
        <h3>応援</h3>
        <div class="about-links">
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
