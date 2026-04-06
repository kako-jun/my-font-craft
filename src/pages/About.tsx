export default function About() {
  return (
    <div class="about-page">
      <h2>MyFontCraft について</h2>

      <div class="card">
        <h3>このサービスについて</h3>
        <p>
          MyFontCraft は、手書きの文字からオリジナルフォントを作成できる無料のWebサービスです。
          テンプレートを印刷して手書きし、スキャンまたは撮影してアップロードするだけで、
          あなた自身の字がフォントファイル（.ttf）になります。
        </p>
        <p>
          すべての処理はブラウザ内で完結します。
          画像がサーバーに送信されることはなく、ユーザー登録も不要です。
        </p>
      </div>

      <div class="card">
        <h3>対応文字</h3>
        <ul class="about-list">
          <li>ひらがな（83文字）</li>
          <li>カタカナ（87文字）</li>
          <li>英数字・記号（102文字）</li>
          <li>常用漢字（2,136文字）</li>
        </ul>
      </div>

      <div class="card">
        <h3>作成したフォントの利用</h3>
        <p>
          作成したフォントはあなたのものです。
          個人利用・商用利用ともに自由にお使いいただけます。
        </p>
      </div>

      <div class="card">
        <h3>作者</h3>
        <p class="about-author">kako-jun</p>
        <div class="about-links">
          <a href="https://llll-ll.com" target="_blank" rel="noopener noreferrer">
            llll-ll.com
          </a>
          <a href="https://github.com/kako-jun/my-font-craft" target="_blank" rel="noopener noreferrer">
            GitHub
          </a>
        </div>
      </div>

      <div class="card">
        <h3>応援する</h3>
        <p>MyFontCraft が気に入ったら、開発を応援していただけると嬉しいです。</p>
        <div class="about-links" style="margin-top:0.75rem">
          <a
            class="btn"
            href="https://github.com/sponsors/kako-jun"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub Sponsors
          </a>
          <a
            class="btn"
            href="https://amzn.to/41dkZF1"
            target="_blank"
            rel="noopener noreferrer"
          >
            Amazon で応援
          </a>
        </div>
      </div>

      <div class="card">
        <h3>ライセンス</h3>
        <p>MIT License</p>
      </div>
    </div>
  );
}
