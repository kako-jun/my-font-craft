export default function IconPen(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* ペン軸（斜めの四角） */}
      <rect x="6" y="1" width="3" height="10" fill="currentColor" transform="rotate(15 8 8)" />
      {/* ペン先 */}
      <rect x="7" y="11" width="1" height="2" fill="currentColor" transform="rotate(15 8 8)" />
      {/* ペン先端の点 */}
      <rect x="7" y="13" width="1" height="1" fill="currentColor" transform="rotate(15 8 8)" />
    </svg>
  );
}
