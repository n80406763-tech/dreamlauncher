import type { ReactNode } from "react";

interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  actions?: ReactNode;
}

export function Dialog({ isOpen, onClose, title, children, actions }: DialogProps) {
  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: "rgba(0, 0, 0, 0.6)" }}
      onClick={onClose}
    >
      <div
        className="panel max-w-lg w-full mx-4"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-labelledby="dialog-title"
      >
        <div className="flex items-center justify-between mb-4">
          <h2
            id="dialog-title"
            className="font-pixel text-base"
            style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}
          >
            {title}
          </h2>
          <button
            className="btn--ghost p-2"
            onClick={onClose}
            aria-label="Закрыть"
          >
            ✕
          </button>
        </div>
        <div className="mb-4" style={{ color: "var(--text)" }}>
          {children}
        </div>
        {actions ? (
          <div className="flex gap-2 justify-end">
            {actions}
          </div>
        ) : null}
      </div>
    </div>
  );
}
