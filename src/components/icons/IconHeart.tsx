export default function IconHeart(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* ドット絵ハート */}
      {/* 上段左 */}
      <rect x="2" y="3" width="4" height="2" fill="currentColor" />
      {/* 上段右 */}
      <rect x="10" y="3" width="4" height="2" fill="currentColor" />
      {/* 中段（つながり） */}
      <rect x="1" y="5" width="14" height="2" fill="currentColor" />
      {/* 下段 */}
      <rect x="2" y="7" width="12" height="2" fill="currentColor" />
      <rect x="3" y="9" width="10" height="2" fill="currentColor" />
      <rect x="5" y="11" width="6" height="2" fill="currentColor" />
      {/* 底点 */}
      <rect x="7" y="13" width="2" height="1" fill="currentColor" />
    </svg>
  );
}
