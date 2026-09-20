// Тонкая обёртка над сгенерированными `commands` из bindings.ts:
// разворачивает { status, data | error } в обычный Promise, который либо
// резолвится значением, либо реджектится `DreamError` — так с командами
// удобно работать через TanStack Query, не проверяя `status` в каждом месте.
import { Channel } from "@tauri-apps/api/core";
import { commands, type DreamError, type InstallProgressEvent, type LaunchEventDto } from "./bindings";

export class IpcError extends Error {
  dreamError: DreamError;

  constructor(dreamError: DreamError) {
    super(dreamError.message);
    this.name = "IpcError";
    this.dreamError = dreamError;
  }
}

async function unwrap<T>(promise: Promise<{ status: "ok"; data: T } | { status: "error"; error: DreamError }>): Promise<T> {
  const result = await promise;
  if (result.status === "error") {
    throw new IpcError(result.error);
  }
  return result.data;
}

export const api = {
  versionsManifest: (refresh: boolean) => unwrap(commands.versionsManifest(refresh)),
  accountsList: () => unwrap(commands.accountsList()),
  accountAddOffline: (name: string) => unwrap(commands.accountAddOffline(name)),
  accountSetActive: (id: string) => unwrap(commands.accountSetActive(id)),
  accountRemove: (id: string) => unwrap(commands.accountRemove(id)),
  instancesList: () => unwrap(commands.instancesList()),
  instanceCreate: (req: Parameters<typeof commands.instanceCreate>[0]) => unwrap(commands.instanceCreate(req)),
  instanceUpdate: (req: Parameters<typeof commands.instanceUpdate>[0]) => unwrap(commands.instanceUpdate(req)),
  instanceOpenFolder: (id: string) => unwrap(commands.instanceOpenFolder(id)),
  instanceDelete: (id: string) => unwrap(commands.instanceDelete(id)),
  loaderVersions: (loader: string, mcVersion: string) => unwrap(commands.loaderVersions(loader, mcVersion)),

  installVersion: (instanceId: string, onEvent: (e: InstallProgressEvent) => void) => {
    const channel = new Channel<InstallProgressEvent>();
    channel.onmessage = onEvent;
    return unwrap(commands.installVersion(instanceId, channel));
  },

  launchInstance: (instanceId: string, onEvent: (e: LaunchEventDto) => void) => {
    const channel = new Channel<LaunchEventDto>();
    channel.onmessage = onEvent;
    return unwrap(commands.launchInstance(instanceId, channel));
  },

  modsSearch: (instanceId: string, query: string, offset: number) => unwrap(commands.modsSearch(instanceId, query, offset)),
  modsList: (instanceId: string) => unwrap(commands.modsList(instanceId)),
  modsInstall: (instanceId: string, projectId: string) => unwrap(commands.modsInstall(instanceId, projectId)),
  modsToggle: (instanceId: string, projectId: string) => unwrap(commands.modsToggle(instanceId, projectId)),
  modsRemove: (instanceId: string, projectId: string) => unwrap(commands.modsRemove(instanceId, projectId)),
  shadersSearch: (instanceId: string, query: string, offset: number) => unwrap(commands.shadersSearch(instanceId, query, offset)),
  shadersList: (instanceId: string) => unwrap(commands.shadersList(instanceId)),
  shadersInstall: (instanceId: string, projectId: string) => unwrap(commands.shadersInstall(instanceId, projectId)),
  shadersSetActive: (instanceId: string, projectId: string | null) => unwrap(commands.shadersSetActive(instanceId, projectId)),
  shadersRemove: (instanceId: string, projectId: string) => unwrap(commands.shadersRemove(instanceId, projectId)),
  crashesList: (instanceId: string) => unwrap(commands.crashesList(instanceId)),
  crashRead: (path: string) => unwrap(commands.crashRead(path)),
};

export type {
  AccountDto,
  InstanceDto,
  CreateInstanceRequest,
  UpdateInstanceRequest,
  VersionManifestDto,
  VersionEntryDto,
  InstallProgressEvent,
  LaunchEventDto,
  ModSearchHitDto,
  ModSearchResultDto,
  InstalledModDto,
  InstalledShaderDto,
  ShaderSearchResultDto,
  CrashReportDto,
} from "./bindings";
