export default function IconImage(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* 額縁 */}
      <rect x="1" y="2" width="14" height="2" fill="currentColor" />
      <rect x="1" y="12" width="14" height="2" fill="currentColor" />
      <rect x="1" y="2" width="2" height="12" fill="currentColor" />
      <rect x="13" y="2" width="2" height="12" fill="currentColor" />
      {/* 山 */}
      <rect x="4" y="9" width="2" height="2" fill="currentColor" />
      <rect x="5" y="8" width="2" height="2" fill="currentColor" />
      <rect x="6" y="7" width="2" height="2" fill="currentColor" />
      <rect x="7" y="8" width="2" height="2" fill="currentColor" />
      <rect x="8" y="9" width="2" height="2" fill="currentColor" />
      <rect x="9" y="10" width="2" height="2" fill="currentColor" />
      {/* 太陽 */}
      <rect x="10" y="5" width="2" height="2" fill="currentColor" />
    </svg>
  );
}
