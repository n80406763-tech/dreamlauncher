import type { HTMLAttributes } from "react";

interface PanelProps extends HTMLAttributes<HTMLDivElement> {
  raised?: boolean;
}

export function Panel({ raised, className = "", ...props }: PanelProps) {
  const classes = ["panel", raised ? "panel--raised" : "", className].filter(Boolean).join(" ");
  return <div className={classes} {...props} />;
}
