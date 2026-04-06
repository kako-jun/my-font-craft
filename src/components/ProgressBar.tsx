interface Props {
  current: number;
  total: number;
  label?: string;
}

export default function ProgressBar(props: Props) {
  const pct = () => Math.round((props.current / props.total) * 100);

  return (
    <div class="progress-wrapper">
      {props.label && <div class="progress-label">{props.label}</div>}
      <div class="progress">
        <div class="progress__fill" style={{ width: `${pct()}%` }} />
      </div>
      <div class="progress-text">
        {props.current}/{props.total} ({pct()}%)
      </div>
    </div>
  );
}
