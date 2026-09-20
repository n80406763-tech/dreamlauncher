import { useState, useEffect } from "react";
import { Button } from "./ui/Button";
import { Panel } from "./ui/Panel";
import { api } from "../ipc/client";
import type { CrashReportDto } from "../ipc/client";
import { message } from "@tauri-apps/plugin-dialog";

export function CrashesPanel({ instanceId }: { instanceId: string }) {
  const [crashes, setCrashes] = useState<CrashReportDto[]>([]);
  const [loading, setLoading] = useState(false);

  const fetchCrashes = async () => {
    setLoading(true);
    const res = await api.crashesList(instanceId);
    if (res.status === "ok") {
      setCrashes(res.data);
    } else {
      await message(`Ошибка загрузки отчетов: ${res.error.message}`, { title: "Ошибка", kind: "error" });
    }
    setLoading(false);
  };

  useEffect(() => {
    fetchCrashes();
  }, [instanceId]);

  const readCrash = async (path: string) => {
    const res = await api.crashRead(path);
    if (res.status === "ok") {
      await message(res.data, { title: "Крэш-репорт", kind: "info" });
    } else {
      await message(`Ошибка чтения: ${res.error.message}`, { title: "Ошибка", kind: "error" });
    }
  };

  return (
    <div className="flex flex-col gap-3 p-2" style={{ background: "var(--bg-slot)" }}>
      <div className="flex items-center justify-between">
        <span className="text-sm" style={{ color: "var(--text-hi)" }}>Свежие вылеты (последние 5)</span>
        <Button size="sm" onClick={fetchCrashes} disabled={loading}>
          Обновить
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        {loading ? (
          <div className="text-sm text-center p-4" style={{ color: "var(--text-dim)" }}>
            Загрузка...
          </div>
        ) : crashes.length === 0 ? (
          <div className="text-sm text-center p-4" style={{ color: "var(--text-dim)" }}>
            Крэш-репортов не найдено. Игра работает стабильно!
          </div>
        ) : (
          crashes.map((crash) => (
            <Panel key={crash.path} className="flex flex-col gap-2 p-3">
              <div className="flex items-center justify-between">
                <div className="flex flex-col">
                  <span className="font-bold text-sm" style={{ color: crash.kind === "jvm_fatal" ? "var(--danger-hi)" : "var(--warn)" }}>
                    {crash.kind === "jvm_fatal" ? "Фатальная ошибка JVM" : "Ошибка игры (Game Crash)"}
                  </span>
                  <span className="text-xs" style={{ color: "var(--text-dim)" }}>
                    {crash.modified} сек. назад
                  </span>
                </div>
                <Button size="sm" onClick={() => readCrash(crash.path)}>
                  Читать полностью
                </Button>
              </div>
              <div className="text-xs font-mono p-2 rounded max-h-24 overflow-y-auto" style={{ background: "var(--bg-void)", color: "var(--text)" }}>
                {crash.preview}
              </div>
            </Panel>
          ))
        )}
      </div>
    </div>
  );
}
