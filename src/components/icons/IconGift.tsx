export default function IconGift(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* リボン上部 */}
      <rect x="4" y="1" width="3" height="3" fill="currentColor" />
      <rect x="9" y="1" width="3" height="3" fill="currentColor" />
      {/* リボン中央 */}
      <rect x="1" y="4" width="14" height="3" fill="currentColor" />
      {/* 箱本体 */}
      <rect
        x="2"
        y="7"
        width="12"
        height="7"
        stroke="currentColor"
        stroke-width="1.5"
        fill="none"
      />
      {/* 縦リボン */}
      <rect x="7" y="7" width="2" height="7" fill="currentColor" />
    </svg>
  );
}
