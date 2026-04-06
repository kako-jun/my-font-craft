export default function IconDownload(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* 矢印の軸 */}
      <rect x="6" y="2" width="4" height="7" fill="currentColor" />
      {/* 矢印の先端 */}
      <rect x="4" y="8" width="8" height="2" fill="currentColor" />
      <rect x="6" y="10" width="4" height="2" fill="currentColor" />
      {/* ベースライン */}
      <rect x="2" y="13" width="12" height="2" fill="currentColor" />
    </svg>
  );
}
