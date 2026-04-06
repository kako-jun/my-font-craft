export default function IconFont(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* 「A」のドット絵 */}
      {/* 頂点 */}
      <rect x="7" y="2" width="2" height="2" fill="currentColor" />
      {/* 左脚 */}
      <rect x="5" y="4" width="2" height="2" fill="currentColor" />
      <rect x="3" y="6" width="2" height="2" fill="currentColor" />
      <rect x="2" y="8" width="2" height="2" fill="currentColor" />
      <rect x="1" y="10" width="2" height="4" fill="currentColor" />
      {/* 右脚 */}
      <rect x="9" y="4" width="2" height="2" fill="currentColor" />
      <rect x="11" y="6" width="2" height="2" fill="currentColor" />
      <rect x="12" y="8" width="2" height="2" fill="currentColor" />
      <rect x="13" y="10" width="2" height="4" fill="currentColor" />
      {/* 横棒 */}
      <rect x="4" y="9" width="8" height="2" fill="currentColor" />
    </svg>
  );
}
