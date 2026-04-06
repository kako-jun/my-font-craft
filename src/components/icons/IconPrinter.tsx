export default function IconPrinter(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* 本体（箱） */}
      <rect x="2" y="5" width="12" height="7" fill="currentColor" />
      {/* 用紙トレイ上部 */}
      <rect x="4" y="2" width="8" height="3" fill="currentColor" />
      {/* 排出口 */}
      <rect x="3" y="7" width="10" height="1" fill="currentColor" />
      {/* 排出される用紙 */}
      <rect x="5" y="10" width="6" height="4" stroke="currentColor" stroke-width="1" fill="none" />
      {/* 用紙の文字行 */}
      <line x1="6" y1="12" x2="10" y2="12" stroke="currentColor" stroke-width="1" />
    </svg>
  );
}
