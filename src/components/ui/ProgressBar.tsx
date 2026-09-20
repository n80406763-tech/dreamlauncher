interface ProgressBarProps {
  value: number; // 0..1
  label?: string;
}

export function ProgressBar({ value, label }: ProgressBarProps) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  return (
    <div>
      <div className="progress" role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100} aria-label={label}>
        <div className="progress__fill" style={{ width: `${pct}%` }} />
      </div>
      {label ? (
        <div className="mt-1 text-xs" style={{ fontFamily: "var(--font-mono)", color: "var(--text-dim)" }}>
          {label}
        </div>
      ) : null}
    </div>
  );
}
