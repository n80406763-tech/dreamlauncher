import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

const appWindow = getCurrentWindow();

/**
 * Своя шапка окна — `decorations: false` в tauri.conf.json убирает системную
 * рамку, а этот компонент отдаёт её обратно: перетаскивание через
 * `-webkit-app-region: drag` (см. styles/components.css `.titlebar`) и три
 * стандартные кнопки управления окном.
 */
export function TitleBar() {
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    appWindow.isMaximized().then(setIsMaximized).catch(() => {});
    const unlisten = appWindow.onResized(() => {
      appWindow.isMaximized().then(setIsMaximized).catch(() => {});
    });
    return () => {
      unlisten.then((f) => f()).catch(() => {});
    };
  }, []);

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="flex items-center gap-2 pl-3 pointer-events-none">
        <span className="font-pixel text-xs tracking-wide" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          DreamLauncher
        </span>
      </div>
      <div className="titlebar__controls">
        <button
          type="button"
          aria-label="Свернуть"
          className="titlebar__btn"
          onClick={() => appWindow.minimize()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
            <rect y="4.5" width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          type="button"
          aria-label={isMaximized ? "Восстановить" : "Развернуть"}
          className="titlebar__btn"
          onClick={() => appWindow.toggleMaximize()}
        >
          {isMaximized ? (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
              <rect x="1.5" y="0.5" width="8" height="8" stroke="currentColor" />
              <rect x="0.5" y="2.5" width="6" height="6" fill="var(--bg-void)" stroke="currentColor" />
            </svg>
          ) : (
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
              <rect x="0.5" y="0.5" width="9" height="9" stroke="currentColor" />
            </svg>
          )}
        </button>
        <button
          type="button"
          aria-label="Закрыть"
          className="titlebar__btn titlebar__btn--close"
          onClick={() => appWindow.close()}
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
            <path d="M0.5 0.5L9.5 9.5M9.5 0.5L0.5 9.5" stroke="currentColor" />
          </svg>
        </button>
      </div>
    </div>
  );
}
