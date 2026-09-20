import { useEffect, useState } from "react";
import { Button } from "./ui/Button";
import { useInstalledShaders, useShaderSearch, useInstallShader, useSetActiveShader, useRemoveShader, SHADER_SEARCH_PAGE_SIZE } from "../hooks/queries";
import type { InstalledShaderDto, ModSearchHitDto } from "../ipc/client";

/// Достаёт slug/ID проекта из вставленного текста — либо это уже сам
/// slug/ID (`bsl-shaders`), либо ссылка на страницу шейдера
/// (`https://modrinth.com/shader/bsl-shaders`).
function extractProjectRef(input: string): string | null {
  const trimmed = input.trim();
  if (!trimmed) return null;
  const match = trimmed.match(/modrinth\.com\/(?:mod|plugin|datapack|shader|resourcepack)\/([^/?#]+)/i);
  if (match) return match[1];
  if (/^[a-zA-Z0-9_-]+$/.test(trimmed)) return trimmed;
  return null;
}

export function ShadersPanel({ instanceId }: { instanceId: string }) {
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

  const installed = useInstalledShaders(instanceId, true);
  const search = useShaderSearch(instanceId, query, offset, true);
  const install = useInstallShader(instanceId);
  const setActive = useSetActiveShader(instanceId);
  const remove = useRemoveShader(instanceId);

  const installedIds = new Set(installed.data?.map((s: InstalledShaderDto) => s.project_id));
  const projectRef = extractProjectRef(directRef);
  const activeShader = installed.data?.find((s: InstalledShaderDto) => s.is_active);

  return (
    <div className="flex flex-col gap-3" style={{ background: "var(--bg-stone)", padding: "var(--sp-3)" }}>
      <div className="flex flex-col gap-1">
        <div className="text-xs" style={{ color: "var(--text-dim)" }}>
          Установленные шейдер-паки
        </div>
        {installed.isLoading ? <div style={{ color: "var(--text-dim)" }}>Загрузка…</div> : null}
        {installed.data?.length === 0 ? <div style={{ color: "var(--text-dim)" }}>Пока ничего не установлено.</div> : null}
        {installed.data?.map((shader: InstalledShaderDto) => (
          <div key={shader.project_id} className="flex items-center justify-between text-sm">
            <div className="flex items-center gap-2 flex-1">
              <input
                type="radio"
                name="active-shader"
                checked={shader.is_active}
                onChange={() => setActive.mutate(shader.project_id)}
                disabled={setActive.isPending}
              />
              <span className="truncate">{shader.filename}</span>
            </div>
            <Button size="sm" variant="danger" onClick={() => remove.mutate(shader.project_id)}>
              ✕
            </Button>
          </div>
        ))}
        {activeShader && (
          <Button size="sm" variant="ghost" onClick={() => setActive.mutate(null)} disabled={setActive.isPending}>
            Отключить шейдеры
          </Button>
        )}
      </div>

      {install.isError && install.error.message.includes("требуется") ? (
        <div className="text-xs p-2" style={{ color: "var(--danger-hi)", background: "var(--bg-void)" }}>
          {install.error.message}
        </div>
      ) : null}

      {/* Установка по ссылке/slug */}
      <div className="flex flex-col gap-1">
        <div className="text-xs" style={{ color: "var(--text-dim)" }}>
          Установить по ссылке или slug (например modrinth.com/shader/bsl-shaders)
        </div>
        <div className="flex gap-2">
          <input className="field flex-1" placeholder="https://modrinth.com/shader/… или просто slug" value={directRef} onChange={(e) => setDirectRef(e.target.value)} />
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
        <input className="field" placeholder="Поиск шейдер-паков на Modrinth…" value={rawQuery} onChange={(e) => setRawQuery(e.target.value)} />
        {search.isFetching ? <div className="text-xs" style={{ color: "var(--text-dim)" }}>Ищем…</div> : null}
        {search.data && search.data.hits.length === 0 ? <div className="text-xs" style={{ color: "var(--text-dim)" }}>Ничего не нашлось для этой версии игры.</div> : null}
        {search.data?.hits.map((hit: ModSearchHitDto) => (
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
        {search.data && (offset > 0 || search.data.hits.length === SHADER_SEARCH_PAGE_SIZE) ? (
          <div className="flex justify-between gap-2">
            <Button size="sm" disabled={offset === 0} onClick={() => setOffset((o) => Math.max(0, o - SHADER_SEARCH_PAGE_SIZE))}>
              ← Назад
            </Button>
            <Button size="sm" disabled={search.data.hits.length < SHADER_SEARCH_PAGE_SIZE} onClick={() => setOffset((o) => o + SHADER_SEARCH_PAGE_SIZE)}>
              Ещё →
            </Button>
          </div>
        ) : null}
        {install.isError && !install.error.message.includes("требуется") ? <div className="text-xs" style={{ color: "var(--danger-hi)" }}>{install.error.message}</div> : null}
      </div>
    </div>
  );
}
