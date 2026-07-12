import { A } from '@solidjs/router';

export default function NotFound() {
  return (
    <div class="not-found">
      <h2>ページが見つかりません</h2>
      <p>お探しのページは存在しません。</p>
      <p class="not-found__actions">
        <A href="/" class="act">
          トップへ戻る
        </A>
      </p>
    </div>
  );
}
