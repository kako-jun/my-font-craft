export default function IconUpload(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* 上矢印 */}
      <rect x="7" y="2" width="2" height="2" fill="currentColor" />
      <rect x="5" y="4" width="2" height="2" fill="currentColor" />
      <rect x="9" y="4" width="2" height="2" fill="currentColor" />
      <rect x="7" y="4" width="2" height="6" fill="currentColor" />
      {/* トレイ */}
      <rect x="2" y="10" width="2" height="4" fill="currentColor" />
      <rect x="12" y="10" width="2" height="4" fill="currentColor" />
      <rect x="2" y="12" width="12" height="2" fill="currentColor" />
    </svg>
  );
}
