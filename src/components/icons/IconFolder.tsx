export default function IconFolder(props: { size?: number; class?: string }) {
  const s = () => props.size ?? 16;
  return (
    <svg width={s()} height={s()} viewBox="0 0 16 16" fill="none" class={props.class}>
      {/* フォルダタブ */}
      <rect x="1" y="3" width="5" height="2" fill="currentColor" />
      {/* フォルダ本体 */}
      <rect x="1" y="5" width="14" height="9" fill="currentColor" />
    </svg>
  );
}
