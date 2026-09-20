import { useEffect, useState } from "react";
import { Button } from "./ui/Button";
import { MOD_SEARCH_PAGE_SIZE, useInstallMod, useInstalledMods, useModSearch, useRemoveMod, useToggleMod } from "../hooks/queries";

/// Достаёт slug/ID проекта из вставленного текста — либо это уже сам
/// slug/ID (`sodium`), либо ссылка на страницу мода
/// (`https://modrinth.com/mod/sodium`, с версией в хвосте или без).
function extractProjectRef(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const match = trimmed.match(/modrinth\.com\/(?:mod|plugin|datapack|shader|resourcepack)\/([^/?#]+)/i);
  if (match) return match[1];
  if (/^[a-zA-Z0-9_-]+$/.test(trimmed)) return trimmed;
  return null;
}

export function ModsPanel({ instanceId }: { instanceId: string }) {
  const [rawQuery, setRawQuery] = useState("");
  const [query, setQuery] = useState("");
  const [offset, setOffset] = useState(0);
  const [directRef, setDirectRef] = useState("");

  // Дебаунс — иначе каждая нажатая буква бьёт по Modrinth API.
  useEffect(() => {
    const t = setTimeout(() => {
      setQuery(rawQuery);
      setOffset(0);
    }, 350);
    return () => clearTimeout(t);
  }, [rawQuery]);

  const installed = useInstalledMods(instanceId, true);
  // Пустой запрос — тоже валидный поиск (Modrinth вернёт популярное),
  // так что панель никогда не выглядит "пустой и сломанной".
  const search = useModSearch(instanceId, query, offset, true);
  const install = useInstallMod(instanceId);
  const toggle = useToggleMod(instanceId);
  const remove = useRemoveMod(instanceId);

  const installedIds = new Set(installed.data?.map((m) => m.project_id));
  const projectRef = extractProjectRef(directRef);

  return (
    <div className="flex flex-col gap-3" style={{ background: "var(--bg-stone)", padding: "var(--sp-3)" }}>
      <div className="flex flex-col gap-1">
        <div className="text-xs" style={{ color: "var(--text-dim)" }}>
          Установленные моды
        </div>
        {installed.isLoading ? <div style={{ color: "var(--text-dim)" }}>Загрузка…</div> : null}
        {installed.data?.length === 0 ? <div style={{ color: "var(--text-dim)" }}>Пока ничего не установлено.</div> : null}
        {installed.data?.map((m) => (
          <div key={m.project_id} className="flex items-center justify-between text-sm" style={{ opacity: m.enabled ? 1 : 0.5 }}>
            <span className="truncate">{m.filename}</span>
            <div className="flex gap-1 shrink-0">
              <Button size="sm" variant="ghost" onClick={() => toggle.mutate(m.project_id)}>
                {m.enabled ? "Выкл" : "Вкл"}
              </Button>
              <Button size="sm" variant="danger" onClick={() => remove.mutate(m.project_id)}>
                ✕
              </Button>
            </div>
          </div>
        ))}
      </div>

      {/* Установка по ссылке/slug — гарантированно достаёт ЛЮБОЙ мод с
          Modrinth, даже если поиск его почему-то не показывает (редкие
          формулировки названия, устаревшая индексация и т.п.). */}
      <div className="flex flex-col gap-1">
        <div className="text-xs" style={{ color: "var(--text-dim)" }}>
          Установить по ссылке или slug (например modrinth.com/mod/sodium)
        </div>
        <div className="flex gap-2">
          <input className="field flex-1" placeholder="https://modrinth.com/mod/… или просто slug" value={directRef} onChange={(e) => setDirectRef(e.target.value)} />
          <Button
            size="sm"
            variant="primary"
            disabled={!projectRef || install.isPending}
            onClick={() => {
              if (projectRef) install.mutate(projectRef, { onSuccess: () => setDirectRef("") });
            }}
          >
            Установить
          </Button>
        </div>
        {directRef && !projectRef ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>Не похоже на ссылку Modrinth или slug</div> : null}
      </div>

      <div className="flex flex-col gap-2">
        <input className="field" placeholder="Поиск модов на Modrinth…" value={rawQuery} onChange={(e) => setRawQuery(e.target.value)} />
        {search.isFetching ? <div className="text-xs" style={{ color: "var(--text-dim)" }}>Ищем…</div> : null}
        {search.data && search.data.hits.length === 0 ? <div className="text-xs" style={{ color: "var(--text-dim)" }}>Ничего не нашлось для этой версии/загрузчика.</div> : null}
        {search.data?.hits.map((hit) => (
          <div key={hit.project_id} className="flex items-center justify-between gap-2 text-sm">
            <div className="min-w-0">
              <div className="truncate" style={{ color: "var(--text-hi)" }}>
                {hit.title}
              </div>
              <div className="truncate text-xs" style={{ color: "var(--text-dim)" }}>
                {hit.description}
              </div>
            </div>
            <Button
              size="sm"
              variant="primary"
              className="shrink-0"
              disabled={installedIds.has(hit.project_id) || install.isPending}
              onClick={() => install.mutate(hit.project_id)}
            >
              {installedIds.has(hit.project_id) ? "Установлен" : "Установить"}
            </Button>
          </div>
        ))}
        {search.data && (offset > 0 || search.data.hits.length === MOD_SEARCH_PAGE_SIZE) ? (
          <div className="flex justify-between gap-2">
            <Button size="sm" disabled={offset === 0} onClick={() => setOffset((o) => Math.max(0, o - MOD_SEARCH_PAGE_SIZE))}>
              ← Назад
            </Button>
            <Button size="sm" disabled={search.data.hits.length < MOD_SEARCH_PAGE_SIZE} onClick={() => setOffset((o) => o + MOD_SEARCH_PAGE_SIZE)}>
              Ещё →
            </Button>
          </div>
        ) : null}
        {install.isError ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>{install.error.message}</div> : null}
      </div>
    </div>
  );
}
