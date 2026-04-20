import { A } from '@solidjs/router';

export default function NotFound() {
  return (
    <div class="card not-found">
      <h2>ページが見つかりません</h2>
      <p>お探しのページは存在しないか、移動された可能性があります。</p>
      <p class="not-found__actions">
        <A href="/" class="btn btn--primary">
          トップへ戻る
        </A>
      </p>
    </div>
  );
}
