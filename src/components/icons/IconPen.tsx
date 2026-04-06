export default function IconPen(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* ペン軸（階段状のドット絵） */}
      <rect x="10" y="1" width="2" height="2" fill="currentColor" />
      <rect x="9" y="3" width="2" height="2" fill="currentColor" />
      <rect x="8" y="5" width="2" height="2" fill="currentColor" />
      <rect x="7" y="7" width="2" height="2" fill="currentColor" />
      <rect x="6" y="9" width="2" height="2" fill="currentColor" />
      {/* ペン先 */}
      <rect x="5" y="11" width="2" height="2" fill="currentColor" />
      {/* ペン先端の点 */}
      <rect x="4" y="13" width="1" height="1" fill="currentColor" />
    </svg>
  );
}
