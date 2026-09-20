import { useEffect, useMemo, useState } from "react";
import { Button } from "../components/ui/Button";
import { Panel } from "../components/ui/Panel";
import { Slot } from "../components/ui/Slot";
import { PlaySessionPanel } from "../components/PlaySessionPanel";
import { InstanceSettingsPanel } from "../components/InstanceSettingsPanel";
import { api } from "../ipc/client";
import type { InstanceDto } from "../ipc/client";
import { useCreateInstance, useDeleteInstance, useInstances, useVersionManifest } from "../hooks/queries";
import { usePlaySession, usePlayStore } from "../stores/play";

const LOADERS = [
  { value: "vanilla", label: "Vanilla" },
  { value: "fabric", label: "Fabric" },
  { value: "quilt", label: "Quilt" },
  { value: "forge", label: "Forge" },
  { value: "neoforge", label: "NeoForge" },
];

function LoaderVersionSelect({ loader, mcVersion, value, onChange }: { loader: string; mcVersion: string; value: string; onChange: (v: string) => void }) {
  const [versions, setVersions] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (loader === "vanilla" || !mcVersion) {
      setVersions([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    api
      .loaderVersions(loader, mcVersion)
      .then((v) => {
        if (!cancelled) {
          setVersions(v);
          if (v.length > 0) onChange(v[0]);
        }
      })
      .catch(() => !cancelled && setVersions([]))
      .finally(() => !cancelled && setLoading(false));
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loader, mcVersion]);

  if (loader === "vanilla") return null;

  return (
    <label className="flex flex-col gap-1 text-xs" style={{ color: "var(--text-dim)" }}>
      Версия загрузчика
      {loading ? (
        <span style={{ color: "var(--text-dim)" }}>Загрузка списка…</span>
      ) : versions.length === 0 ? (
        <span style={{ color: "var(--danger-hi)" }}>Нет сборок для этой версии Minecraft</span>
      ) : (
        <select className="field" value={value} onChange={(e) => onChange(e.target.value)}>
          {versions.map((v) => (
            <option key={v} value={v}>
              {v}
            </option>
          ))}
        </select>
      )}
    </label>
  );
}

function InstanceCard({ inst, onDelete, deleting }: { inst: InstanceDto; onDelete: () => void; deleting: boolean }) {
  const session = usePlaySession(inst.id);
  const play = usePlayStore((s) => s.play);
  const busy = session.phase === "installing" || session.phase === "launching" || session.phase === "running";
  const [showSettings, setShowSettings] = useState(false);

  return (
    <Panel raised className="flex flex-col gap-2">
      <div className="flex items-center gap-3">
        <Slot size="lg" />
        <div className="flex flex-col min-w-0">
          <span className="truncate" style={{ color: "var(--text-hi)" }}>
            {inst.name}
          </span>
          <span className="text-xs" style={{ color: "var(--text-dim)" }}>
            {inst.mc_version} · {inst.loader}
            {inst.loader_version ? ` ${inst.loader_version}` : ""}
          </span>
        </div>
      </div>
      <div className="flex gap-2">
        <Button size="sm" variant="primary" className="flex-1" disabled={busy} onClick={() => play(inst.id)}>
          {busy ? "Играем…" : "Играть"}
        </Button>
        <Button size="sm" onClick={() => setShowSettings((v) => !v)}>
          {showSettings ? "Скрыть" : "⚙"}
        </Button>
        <Button size="sm" variant="danger" disabled={busy || deleting} onClick={onDelete}>
          ✕
        </Button>
      </div>
      <PlaySessionPanel session={session} />
      {showSettings ? <InstanceSettingsPanel inst={inst} /> : null}
    </Panel>
  );
}

export function InstancesScreen() {
  const { data: instances, isLoading } = useInstances();
  const { data: manifest } = useVersionManifest();
  const createInstance = useCreateInstance();
  const deleteInstance = useDeleteInstance();

  const [showForm, setShowForm] = useState(false);
  const [name, setName] = useState("");
  const [loader, setLoader] = useState("vanilla");
  const [loaderVersion, setLoaderVersion] = useState("");

  const releaseVersions = useMemo(() => manifest?.versions.filter((v) => v.kind === "release").map((v) => v.id) ?? [], [manifest]);
  const [mcVersion, setMcVersion] = useState("");

  const effectiveVersion = mcVersion || releaseVersions[0] || "";
  const canSubmit = !!name && !!effectiveVersion && (loader === "vanilla" || !!loaderVersion);

  return (
    <div className="flex flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <h1 className="font-pixel text-lg" style={{ fontFamily: "var(--font-pixel)", color: "var(--text-hi)" }}>
          Инстансы
        </h1>
        <Button variant="primary" onClick={() => setShowForm((v) => !v)}>
          {showForm ? "Отмена" : "+ Новый инстанс"}
        </Button>
      </div>

      {showForm ? (
        <Panel className="flex flex-col gap-3 max-w-xl">
          <form
            className="flex flex-col gap-3"
            onSubmit={(e) => {
              e.preventDefault();
              if (!canSubmit) return;
              createInstance.mutate(
                { name, mc_version: effectiveVersion, loader, loader_version: loader === "vanilla" ? null : loaderVersion },
                { onSuccess: () => { setShowForm(false); setName(""); } },
              );
            }}
          >
            <label className="flex flex-col gap-1 text-xs" style={{ color: "var(--text-dim)" }}>
              Название
              <input className="field" value={name} onChange={(e) => setName(e.target.value)} placeholder="Моя сборка" autoFocus />
            </label>
            <label className="flex flex-col gap-1 text-xs" style={{ color: "var(--text-dim)" }}>
              Версия Minecraft
              <select className="field" value={effectiveVersion} onChange={(e) => setMcVersion(e.target.value)}>
                {releaseVersions.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex flex-col gap-1 text-xs" style={{ color: "var(--text-dim)" }}>
              Загрузчик
              <select className="field" value={loader} onChange={(e) => { setLoader(e.target.value); setLoaderVersion(""); }}>
                {LOADERS.map((l) => (
                  <option key={l.value} value={l.value}>
                    {l.label}
                  </option>
                ))}
              </select>
            </label>
            <LoaderVersionSelect loader={loader} mcVersion={effectiveVersion} value={loaderVersion} onChange={setLoaderVersion} />
            {createInstance.isError ? <div style={{ color: "var(--danger-hi)" }}>{createInstance.error.message}</div> : null}
            <Button type="submit" variant="primary" disabled={!canSubmit || createInstance.isPending}>
              {createInstance.isPending ? "Создаём…" : "Создать"}
            </Button>
          </form>
        </Panel>
      ) : null}

      <div className="grid gap-3" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(220px, 1fr))" }}>
        {isLoading ? <div style={{ color: "var(--text-dim)" }}>Загрузка…</div> : null}
        {instances?.length === 0 && !showForm ? <div style={{ color: "var(--text-dim)" }}>Инстансов пока нет — создайте первый.</div> : null}
        {instances?.map((inst) => (
          <InstanceCard key={inst.id} inst={inst} onDelete={() => deleteInstance.mutate(inst.id)} deleting={deleteInstance.isPending} />
        ))}
      </div>
    </div>
  );
}
