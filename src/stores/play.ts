// Состояние "играется прямо сейчас" по каждому инстансу отдельно —
// установка + запуск дальше живут здесь, а не в компоненте экрана, чтобы
// не терять прогресс при переключении вкладок (Играть <-> Инстансы).
import { create } from "zustand";
import { api, IpcError, type InstallProgressEvent, type LaunchEventDto } from "../ipc/client";

export type PlayPhase = "idle" | "installing" | "launching" | "running" | "exited" | "error";

export interface PlaySession {
  phase: PlayPhase;
  message: string;
  progress?: { done: number; total: number };
  log: string[];
  exitCode?: number | null;
}

const MAX_LOG_LINES = 300;

interface PlayStore {
  sessions: Record<string, PlaySession>;
  play: (instanceId: string) => Promise<void>;
}

const idle: PlaySession = { phase: "idle", message: "", log: [] };

export const usePlayStore = create<PlayStore>((set, get) => ({
  sessions: {},

  play: async (instanceId: string) => {
    const current = get().sessions[instanceId];
    if (current && (current.phase === "installing" || current.phase === "launching" || current.phase === "running")) {
      return; // уже играем/устанавливаем — повторный клик игнорируем
    }

    const patch = (p: Partial<PlaySession>) =>
      set((s) => ({ sessions: { ...s.sessions, [instanceId]: { ...(s.sessions[instanceId] ?? idle), ...p } } }));

    patch({ phase: "installing", message: "Готовим установку…", log: [], progress: undefined, exitCode: undefined });

    try {
      await api.installVersion(instanceId, (event: InstallProgressEvent) => {
        switch (event.type) {
          case "Phase":
            patch({ message: event.message });
            break;
          case "Progress":
            patch({ progress: { done: event.done, total: event.total } });
            break;
          case "ItemFailed":
            patch({ message: `Ошибка файла: ${event.url}` });
            break;
          case "Error":
            patch({ phase: "error", message: event.message });
            break;
          case "Done":
            break;
        }
      });
    } catch (e) {
      patch({ phase: "error", message: e instanceof IpcError ? e.message : String(e) });
      return;
    }

    patch({ phase: "launching", message: "Запускаем игру…" });

    try {
      await api.launchInstance(instanceId, (event: LaunchEventDto) => {
        switch (event.type) {
          case "Starting":
            patch({ phase: "running", message: "Игра запущена" });
            break;
          case "Stdout":
          case "Stderr": {
            const line = event.line;
            set((s) => {
              const session = s.sessions[instanceId] ?? idle;
              const log = [...session.log, line].slice(-MAX_LOG_LINES);
              return { sessions: { ...s.sessions, [instanceId]: { ...session, log } } };
            });
            break;
          }
          case "Exited":
            patch({ phase: "exited", message: event.code === 0 ? "Игра закрыта" : `Игра завершилась с кодом ${event.code}`, exitCode: event.code });
            break;
          case "Error":
            patch({ phase: "error", message: event.message });
            break;
        }
      });
    } catch (e) {
      patch({ phase: "error", message: e instanceof IpcError ? e.message : String(e) });
    }
  },
}));

export function usePlaySession(instanceId: string): PlaySession {
  return usePlayStore((s) => s.sessions[instanceId] ?? idle);
}
