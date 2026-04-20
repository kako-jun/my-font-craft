import { A } from '@solidjs/router';

export default function NotFound() {
  return (
    <div class="card" style="margin-top:2rem;text-align:center">
      <h2>ページが見つかりません</h2>
      <p>お探しのページは存在しないか、移動された可能性があります。</p>
      <p style="margin-top:1rem">
        <A href="/" class="btn btn--primary">
          トップへ戻る
        </A>
      </p>
    </div>
  );
}
