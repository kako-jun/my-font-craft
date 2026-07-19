import { A } from '@solidjs/router';

export default function NotFound() {
  return (
    <div>
      <h1>ページが見つかりません</h1>
      <section class="page-section">
        <h2>移動先</h2>
        <div class="section__body">
          <p>お探しのページは存在しません。</p>
          <A href="/" class="act">
            トップへ戻る
          </A>
        </div>
      </section>
    </div>
  );
}
