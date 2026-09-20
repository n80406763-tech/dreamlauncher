import { useState } from "react";
import { Button } from "./ui/Button";
import { useOpenInstanceFolder, useUpdateInstance } from "../hooks/queries";
import type { InstanceDto } from "../ipc/client";
import { ModsPanel } from "./ModsPanel";
import { ShadersPanel } from "./ShadersPanel";

type InstanceTab = "settings" | "mods" | "shaders";

/// RAM и JVM-аргументы инстанса — правится тут; версия игры/загрузчик не
/// редактируются (см. `UpdateInstanceRequest` на бэкенде: это фактически
/// новый инстанс, проще пересоздать).
export function InstanceSettingsPanel({ inst }: { inst: InstanceDto }) {
  const [activeTab, setActiveTab] = useState<InstanceTab>("settings");
  const [minRam, setMinRam] = useState(inst.min_ram_mb);
  const [maxRam, setMaxRam] = useState(inst.max_ram_mb);
  const [jvmArgs, setJvmArgs] = useState(inst.extra_jvm_args);
  const update = useUpdateInstance();
  const openFolder = useOpenInstanceFolder();

  const dirty = minRam !== inst.min_ram_mb || maxRam !== inst.max_ram_mb || jvmArgs !== inst.extra_jvm_args;
  const invalid = minRam <= 0 || minRam > maxRam;

  return (
    <div className="flex flex-col gap-2">
      {/* Вкладки */}
      <div className="flex gap-2" style={{ borderBottom: "1px solid var(--bg-stone)" }}>
        <button
          className={`px-3 py-2 text-sm ${activeTab === "settings" ? "font-bold" : ""}`}
          style={{
            color: activeTab === "settings" ? "var(--text-hi)" : "var(--text-dim)",
            borderBottom: activeTab === "settings" ? "2px solid var(--accent)" : "none"
          }}
          onClick={() => setActiveTab("settings")}
        >
          Настройки
        </button>
        <button
          className={`px-3 py-2 text-sm ${activeTab === "mods" ? "font-bold" : ""}`}
          style={{
            color: activeTab === "mods" ? "var(--text-hi)" : "var(--text-dim)",
            borderBottom: activeTab === "mods" ? "2px solid var(--accent)" : "none"
          }}
          onClick={() => setActiveTab("mods")}
        >
          Моды
        </button>
        <button
          className={`px-3 py-2 text-sm ${activeTab === "shaders" ? "font-bold" : ""}`}
          style={{
            color: activeTab === "shaders" ? "var(--text-hi)" : "var(--text-dim)",
            borderBottom: activeTab === "shaders" ? "2px solid var(--accent)" : "none"
          }}
          onClick={() => setActiveTab("shaders")}
        >
          Шейдеры
        </button>
      </div>

      {/* Содержимое вкладок */}
      {activeTab === "settings" && (
        <div className="flex flex-col gap-3 p-2" style={{ background: "var(--bg-slot)" }}>
          <div className="flex flex-col gap-1">
            <div className="flex justify-between text-xs" style={{ color: "var(--text-dim)" }}>
              <span>RAM: {minRam} – {maxRam} МБ</span>
            </div>
            <div className="flex gap-2 items-center">
              <span className="text-xs w-16" style={{ color: "var(--text-dim)" }}>Мин.</span>
              <input type="range" min={256} max={maxRam} step={256} value={minRam} onChange={(e) => setMinRam(Number(e.target.value))} className="flex-1" />
            </div>
            <div className="flex gap-2 items-center">
              <span className="text-xs w-16" style={{ color: "var(--text-dim)" }}>Макс.</span>
              <input type="range" min={minRam} max={16384} step={256} value={maxRam} onChange={(e) => setMaxRam(Number(e.target.value))} className="flex-1" />
            </div>
          </div>

          <label className="flex flex-col gap-1 text-xs" style={{ color: "var(--text-dim)" }}>
            Доп. аргументы JVM
            <input className="field" value={jvmArgs} onChange={(e) => setJvmArgs(e.target.value)} placeholder="например -Dfoo=bar" />
          </label>

          {invalid ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>Минимум RAM должен быть больше 0 и не больше максимума</div> : null}
          {update.isError ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>{update.error.message}</div> : null}
          {openFolder.isError ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>{openFolder.error.message}</div> : null}

          <div className="flex gap-2">
            <Button size="sm" variant="primary" disabled={!dirty || invalid || update.isPending} onClick={() => update.mutate({ id: inst.id, min_ram_mb: minRam, max_ram_mb: maxRam, extra_jvm_args: jvmArgs })}>
              {update.isPending ? "Сохраняем…" : "Сохранить"}
            </Button>
            <Button size="sm" disabled={openFolder.isPending} onClick={() => openFolder.mutate(inst.id)}>
              Открыть папку
            </Button>
          </div>
        </div>
      )}

      {activeTab === "mods" && <ModsPanel instanceId={inst.id} />}
      {activeTab === "shaders" && <ShadersPanel instanceId={inst.id} />}
    </div>
  );
}
