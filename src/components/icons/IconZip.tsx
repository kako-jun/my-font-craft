export default function IconZip(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* ファイル本体 */}
      <rect x="3" y="1" width="10" height="2" fill="currentColor" />
      <rect x="3" y="1" width="2" height="14" fill="currentColor" />
      <rect x="11" y="1" width="2" height="14" fill="currentColor" />
      <rect x="3" y="13" width="10" height="2" fill="currentColor" />
      {/* ファスナー（中央の縞） */}
      <rect x="7" y="2" width="2" height="2" fill="currentColor" />
      <rect x="7" y="5" width="2" height="2" fill="currentColor" />
      <rect x="7" y="8" width="2" height="2" fill="currentColor" />
      <rect x="7" y="11" width="2" height="2" fill="currentColor" />
    </svg>
  );
}
