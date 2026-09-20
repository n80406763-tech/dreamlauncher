import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "danger" | "ghost";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "md" | "sm";
}

const variantClass: Record<Variant, string> = {
  primary: "btn--primary",
  secondary: "",
  danger: "btn--danger",
  ghost: "btn--ghost",
};

export function Button({ variant = "secondary", size = "md", className = "", ...props }: ButtonProps) {
  const classes = ["btn", variantClass[variant], size === "sm" ? "btn--sm" : "", className].filter(Boolean).join(" ");
  return <button className={classes} {...props} />;
}
