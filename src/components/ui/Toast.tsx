import { useEffect, useState } from "react";

export type ToastVariant = "info" | "success" | "warning" | "error";

interface Toast {
  id: string;
  message: string;
  variant: ToastVariant;
}

let toasts: Toast[] = [];
let listeners: Array<(toasts: Toast[]) => void> = [];

export const toast = {
  show(message: string, variant: ToastVariant = "info") {
    const id = `toast-${Date.now()}-${Math.random()}`;
    const newToast = { id, message, variant };
    toasts = [...toasts, newToast];
    listeners.forEach((fn) => fn(toasts));

    // Автоматически убираем через 4 секунды
    setTimeout(() => {
      toasts = toasts.filter((t) => t.id !== id);
      listeners.forEach((fn) => fn(toasts));
    }, 4000);
  },
  info(message: string) {
    this.show(message, "info");
  },
  success(message: string) {
    this.show(message, "success");
  },
  warning(message: string) {
    this.show(message, "warning");
  },
  error(message: string) {
    this.show(message, "error");
  },
};

export function ToastContainer() {
  const [current, setCurrent] = useState<Toast[]>([]);

  useEffect(() => {
    const listener = (newToasts: Toast[]) => setCurrent(newToasts);
    listeners.push(listener);
    return () => {
      listeners = listeners.filter((l) => l !== listener);
    };
  }, []);

  if (current.length === 0) return null;

  const variantStyles: Record<ToastVariant, { bg: string; color: string }> = {
    info: { bg: "var(--info)", color: "var(--text-hi)" },
    success: { bg: "var(--xp)", color: "var(--bg-void)" },
    warning: { bg: "var(--warn)", color: "var(--bg-void)" },
    error: { bg: "var(--danger)", color: "var(--text-hi)" },
  };

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      {current.map((t) => {
        const style = variantStyles[t.variant];
        return (
          <div
            key={t.id}
            className="panel px-4 py-3 shadow-lg"
            style={{
              background: style.bg,
              color: style.color,
              animation: "slideInRight 150ms steps(4)",
            }}
          >
            {t.message}
          </div>
        );
      })}
    </div>
  );
}
