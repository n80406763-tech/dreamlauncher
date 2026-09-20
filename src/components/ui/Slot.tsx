import type { HTMLAttributes } from "react";

interface SlotProps extends HTMLAttributes<HTMLDivElement> {
  size?: "md" | "lg";
}

export function Slot({ size = "md", className = "", ...props }: SlotProps) {
  const classes = ["slot", size === "lg" ? "slot--lg" : "", className].filter(Boolean).join(" ");
  return <div className={classes} {...props} />;
}
