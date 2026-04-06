export default function About() {
  return (
    <div class="about-page">
      {/* Hero */}
      <section class="about-section about-hero">
        <div class="about-section__inner">
          <h2 class="hero__title" style="text-align:center">
            MyFontCraft
          </h2>
          <p style="text-align:center; font-size:1.1rem; color:var(--accent-light)">
            手書きの文字が、あなただけのフォントになる
          </p>
          <div class="about-separator" />
        </div>
      </section>

      {/* Workflow */}
      <section class="about-section">
        <div class="about-section__inner">
          <h3 style="text-align:center; color:var(--accent); margin-bottom:1.5rem">
            フォント作成の流れ
          </h3>
          <div class="about-workflow">
            <div class="about-workflow__step">
              <div class="about-workflow__icon">
                <svg
                  width="48"
                  height="48"
                  viewBox="0 0 48 48"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Printer icon - pixel art style */}
                  <rect x="12" y="8" width="24" height="4" fill="var(--accent)" />
                  <rect x="12" y="12" width="4" height="12" fill="var(--accent)" />
                  <rect x="32" y="12" width="4" height="12" fill="var(--accent)" />
                  <rect x="8" y="24" width="32" height="4" fill="var(--accent)" />
                  <rect x="8" y="28" width="4" height="8" fill="var(--accent)" />
                  <rect x="36" y="28" width="4" height="8" fill="var(--accent)" />
                  <rect x="8" y="36" width="32" height="4" fill="var(--accent)" />
                  <rect x="16" y="12" width="16" height="4" fill="var(--accent-light)" />
                </svg>
              </div>
              <div class="about-workflow__label">1. 印刷</div>
              <div class="about-workflow__desc">テンプレートを印刷</div>
            </div>

            <div class="about-workflow__arrow">
              <svg width="32" height="16" viewBox="0 0 32 16" fill="none" aria-hidden="true">
                <rect x="0" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="8" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="16" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="2" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="10" width="4" height="4" fill="var(--accent-light)" />
                <rect x="28" y="6" width="4" height="4" fill="var(--accent-light)" />
              </svg>
            </div>

            <div class="about-workflow__step">
              <div class="about-workflow__icon">
                <svg
                  width="48"
                  height="48"
                  viewBox="0 0 48 48"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Pen icon - pixel art style */}
                  <rect x="32" y="8" width="4" height="4" fill="var(--accent)" />
                  <rect x="28" y="12" width="4" height="4" fill="var(--accent)" />
                  <rect x="24" y="16" width="4" height="4" fill="var(--accent)" />
                  <rect x="20" y="20" width="4" height="4" fill="var(--accent)" />
                  <rect x="16" y="24" width="4" height="4" fill="var(--accent)" />
                  <rect x="12" y="28" width="4" height="4" fill="var(--accent)" />
                  <rect x="8" y="32" width="4" height="4" fill="var(--accent)" />
                  <rect x="8" y="36" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="12" y="36" width="4" height="4" fill="var(--accent-light)" />
                </svg>
              </div>
              <div class="about-workflow__label">2. 手書き</div>
              <div class="about-workflow__desc">マスに文字を書く</div>
            </div>

            <div class="about-workflow__arrow">
              <svg width="32" height="16" viewBox="0 0 32 16" fill="none" aria-hidden="true">
                <rect x="0" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="8" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="16" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="2" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="10" width="4" height="4" fill="var(--accent-light)" />
                <rect x="28" y="6" width="4" height="4" fill="var(--accent-light)" />
              </svg>
            </div>

            <div class="about-workflow__step">
              <div class="about-workflow__icon">
                <svg
                  width="48"
                  height="48"
                  viewBox="0 0 48 48"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Camera icon - pixel art style */}
                  <rect x="16" y="12" width="8" height="4" fill="var(--accent)" />
                  <rect x="8" y="16" width="32" height="4" fill="var(--accent)" />
                  <rect x="8" y="20" width="4" height="16" fill="var(--accent)" />
                  <rect x="36" y="20" width="4" height="16" fill="var(--accent)" />
                  <rect x="8" y="36" width="32" height="4" fill="var(--accent)" />
                  <rect x="20" y="24" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="24" y="24" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="20" y="28" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="24" y="28" width="4" height="4" fill="var(--accent-light)" />
                </svg>
              </div>
              <div class="about-workflow__label">3. スキャン</div>
              <div class="about-workflow__desc">撮影してアップロード</div>
            </div>

            <div class="about-workflow__arrow">
              <svg width="32" height="16" viewBox="0 0 32 16" fill="none" aria-hidden="true">
                <rect x="0" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="8" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="16" y="6" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="2" width="4" height="4" fill="var(--accent-light)" />
                <rect x="24" y="10" width="4" height="4" fill="var(--accent-light)" />
                <rect x="28" y="6" width="4" height="4" fill="var(--accent-light)" />
              </svg>
            </div>

            <div class="about-workflow__step">
              <div class="about-workflow__icon">
                <svg
                  width="48"
                  height="48"
                  viewBox="0 0 48 48"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Font/document icon - pixel art style */}
                  <rect x="12" y="8" width="20" height="4" fill="var(--accent)" />
                  <rect x="12" y="12" width="4" height="28" fill="var(--accent)" />
                  <rect x="28" y="8" width="4" height="4" fill="var(--accent)" />
                  <rect x="32" y="12" width="4" height="28" fill="var(--accent)" />
                  <rect x="12" y="36" width="24" height="4" fill="var(--accent)" />
                  <rect x="18" y="18" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="22" y="14" width="4" height="12" fill="var(--accent-light)" />
                  <rect x="26" y="18" width="4" height="4" fill="var(--accent-light)" />
                  <rect x="18" y="22" width="12" height="4" fill="var(--accent-light)" />
                </svg>
              </div>
              <div class="about-workflow__label">4. 完成</div>
              <div class="about-workflow__desc">フォントファイルを取得</div>
            </div>
          </div>
        </div>
      </section>

      {/* Characters */}
      <section class="about-section">
        <div class="about-section__inner">
          <h3 style="text-align:center; color:var(--accent); margin-bottom:1.5rem">対応文字</h3>
          <div class="about-chars">
            <div class="about-chars__group">
              <div class="about-chars__title">ひらがな（83文字）</div>
              <div class="about-chars__sample">あいうえおかきくけこさしすせそ...</div>
            </div>
            <div class="about-chars__group">
              <div class="about-chars__title">カタカナ（87文字）</div>
              <div class="about-chars__sample">アイウエオカキクケコサシスセソ...</div>
            </div>
            <div class="about-chars__group">
              <div class="about-chars__title">英数字・記号（102文字）</div>
              <div class="about-chars__sample">ABCabc123!?@#...</div>
            </div>
            <div class="about-chars__group">
              <div class="about-chars__title">常用漢字（2,136文字）</div>
              <div class="about-chars__sample">亜哀挨愛曖悪握圧扱宛嵐安...</div>
            </div>
          </div>
        </div>
      </section>

      {/* Features */}
      <section class="about-section">
        <div class="about-section__inner">
          <h3 style="text-align:center; color:var(--accent); margin-bottom:1.5rem">
            サービスの特長
          </h3>
          <div class="about-features">
            <div class="card" style="text-align:center">
              <div class="about-feature-icon">
                <svg
                  width="40"
                  height="40"
                  viewBox="0 0 40 40"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Browser icon */}
                  <rect x="4" y="8" width="32" height="4" fill="var(--accent)" />
                  <rect x="4" y="12" width="4" height="20" fill="var(--accent)" />
                  <rect x="32" y="12" width="4" height="20" fill="var(--accent)" />
                  <rect x="4" y="32" width="32" height="4" fill="var(--accent)" />
                  <rect x="8" y="9" width="3" height="2" fill="var(--accent-highlight)" />
                  <rect x="12" y="9" width="3" height="2" fill="var(--accent-highlight)" />
                </svg>
              </div>
              <h4 style="color:var(--accent); margin:0.5rem 0 0.25rem">ブラウザ完結</h4>
              <p style="font-size:0.9rem; color:var(--accent-light)">
                インストール不要。ブラウザだけで使えます
              </p>
            </div>
            <div class="card" style="text-align:center">
              <div class="about-feature-icon">
                <svg
                  width="40"
                  height="40"
                  viewBox="0 0 40 40"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Shield/lock icon */}
                  <rect x="16" y="4" width="8" height="4" fill="var(--accent)" />
                  <rect x="12" y="8" width="4" height="8" fill="var(--accent)" />
                  <rect x="24" y="8" width="4" height="8" fill="var(--accent)" />
                  <rect x="8" y="16" width="24" height="4" fill="var(--accent)" />
                  <rect x="8" y="20" width="4" height="12" fill="var(--accent)" />
                  <rect x="28" y="20" width="4" height="12" fill="var(--accent)" />
                  <rect x="12" y="32" width="16" height="4" fill="var(--accent)" />
                  <rect x="18" y="24" width="4" height="4" fill="var(--accent-highlight)" />
                </svg>
              </div>
              <h4 style="color:var(--accent); margin:0.5rem 0 0.25rem">サーバー送信なし</h4>
              <p style="font-size:0.9rem; color:var(--accent-light)">
                画像データは端末の外に出ません
              </p>
            </div>
            <div class="card" style="text-align:center">
              <div class="about-feature-icon">
                <svg
                  width="40"
                  height="40"
                  viewBox="0 0 40 40"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Person/no-register icon */}
                  <rect x="16" y="4" width="8" height="4" fill="var(--accent)" />
                  <rect x="14" y="8" width="12" height="4" fill="var(--accent)" />
                  <rect x="16" y="12" width="8" height="4" fill="var(--accent)" />
                  <rect x="18" y="16" width="4" height="4" fill="var(--accent)" />
                  <rect x="10" y="20" width="20" height="4" fill="var(--accent)" />
                  <rect x="12" y="24" width="4" height="8" fill="var(--accent)" />
                  <rect x="24" y="24" width="4" height="8" fill="var(--accent)" />
                  <rect x="12" y="32" width="4" height="4" fill="var(--accent)" />
                  <rect x="24" y="32" width="4" height="4" fill="var(--accent)" />
                </svg>
              </div>
              <h4 style="color:var(--accent); margin:0.5rem 0 0.25rem">ユーザー登録不要</h4>
              <p style="font-size:0.9rem; color:var(--accent-light)">
                アカウント作成なしですぐ使えます
              </p>
            </div>
            <div class="card" style="text-align:center">
              <div class="about-feature-icon">
                <svg
                  width="40"
                  height="40"
                  viewBox="0 0 40 40"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                  aria-hidden="true"
                >
                  {/* Free/gift icon */}
                  <rect x="8" y="16" width="24" height="4" fill="var(--accent)" />
                  <rect x="8" y="20" width="4" height="16" fill="var(--accent)" />
                  <rect x="28" y="20" width="4" height="16" fill="var(--accent)" />
                  <rect x="8" y="36" width="24" height="4" fill="var(--accent)" />
                  <rect x="18" y="16" width="4" height="24" fill="var(--accent-highlight)" />
                  <rect x="14" y="8" width="4" height="8" fill="var(--accent)" />
                  <rect x="22" y="8" width="4" height="8" fill="var(--accent)" />
                  <rect x="18" y="12" width="4" height="4" fill="var(--accent-highlight)" />
                </svg>
              </div>
              <h4 style="color:var(--accent); margin:0.5rem 0 0.25rem">無料</h4>
              <p style="font-size:0.9rem; color:var(--accent-light)">
                すべての機能を無料で利用できます
              </p>
            </div>
          </div>
          <p style="text-align:center; margin-top:1.5rem; font-size:0.95rem; color:var(--accent-light)">
            作成したフォントはあなたのものです。個人利用・商用利用ともに自由にお使いいただけます。
          </p>
        </div>
      </section>

      {/* Author */}
      <section class="about-section">
        <div class="about-section__inner">
          <h3 style="text-align:center; color:var(--accent); margin-bottom:1.5rem">作者</h3>
          <div class="about-author-card">
            <img
              src="https://github.com/kako-jun.png"
              alt="kako-jun"
              width="80"
              height="80"
              class="about-author-card__avatar"
            />
            <div class="about-author-card__info">
              <p class="about-author">kako-jun</p>
              <div class="about-links">
                <a href="https://llll-ll.com" target="_blank" rel="noopener noreferrer">
                  llll-ll.com
                </a>
                <a
                  href="https://github.com/kako-jun/my-font-craft"
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  GitHub
                </a>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Support */}
      <section class="about-section">
        <div class="about-section__inner" style="text-align:center">
          <h3 style="color:var(--accent); margin-bottom:0.5rem">応援する</h3>
          <p style="margin-bottom:1rem; color:var(--accent-light); font-size:0.95rem">
            このサービスは個人で開発しています。気に入ったら応援していただけると嬉しいです。
          </p>
          <div class="about-links" style="justify-content:center">
            <a
              class="btn"
              href="https://github.com/sponsors/kako-jun"
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub Sponsors
            </a>
            <a class="btn" href="https://amzn.to/41dkZF1" target="_blank" rel="noopener noreferrer">
              Amazon で応援
            </a>
          </div>
        </div>
      </section>
    </div>
  );
}
