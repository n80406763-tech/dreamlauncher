// Хуки TanStack Query поверх `api` — вся серверная (Rust-side) IPC-логика
// живёт здесь, компоненты экранов дальше знают только про хуки.
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type CreateInstanceRequest, type UpdateInstanceRequest } from "../ipc/client";

export function useVersionManifest() {
  return useQuery({
    queryKey: ["versions-manifest"],
    queryFn: () => api.versionsManifest(false),
    staleTime: 5 * 60 * 1000,
  });
}

export function useAccounts() {
  return useQuery({
    queryKey: ["accounts"],
    queryFn: api.accountsList,
    staleTime: 30 * 1000,
  });
}

export function useAddOfflineAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.accountAddOffline(name),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["accounts"] }),
  });
}

export function useSetActiveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.accountSetActive(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["accounts"] }),
  });
}

export function useRemoveAccount() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.accountRemove(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["accounts"] }),
  });
}

export function useInstances() {
  return useQuery({
    queryKey: ["instances"],
    queryFn: api.instancesList,
    staleTime: 30 * 1000,
  });
}

export function useCreateInstance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: CreateInstanceRequest) => api.instanceCreate(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["instances"] }),
  });
}

export function useDeleteInstance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.instanceDelete(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["instances"] }),
  });
}

export function useUpdateInstance() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: UpdateInstanceRequest) => api.instanceUpdate(req),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["instances"] }),
  });
}

export function useOpenInstanceFolder() {
  return useMutation({
    mutationFn: (id: string) => api.instanceOpenFolder(id),
  });
}

export function useInstalledMods(instanceId: string, enabled: boolean) {
  return useQuery({
    queryKey: ["mods", instanceId],
    queryFn: () => api.modsList(instanceId),
    enabled,
  });
}

export const MOD_SEARCH_PAGE_SIZE = 20;

export function useModSearch(instanceId: string, query: string, offset: number, enabled: boolean) {
  return useQuery({
    queryKey: ["mods-search", instanceId, query, offset],
    queryFn: () => api.modsSearch(instanceId, query, offset),
    enabled,
    staleTime: 60 * 1000,
    placeholderData: (prev) => prev,
  });
}

export function useInstallMod(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.modsInstall(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["mods", instanceId] }),
  });
}

export function useToggleMod(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.modsToggle(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["mods", instanceId] }),
  });
}

export function useRemoveMod(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.modsRemove(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["mods", instanceId] }),
  });
}

export function useInstalledShaders(instanceId: string, enabled: boolean) {
  return useQuery({
    queryKey: ["shaders", instanceId],
    queryFn: () => api.shadersList(instanceId),
    enabled,
  });
}

export const SHADER_SEARCH_PAGE_SIZE = 20;

export function useShaderSearch(instanceId: string, query: string, offset: number, enabled: boolean) {
  return useQuery({
    queryKey: ["shaders-search", instanceId, query, offset],
    queryFn: () => api.shadersSearch(instanceId, query, offset),
    enabled,
    staleTime: 60 * 1000,
    placeholderData: (prev) => prev,
  });
}

export function useInstallShader(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.shadersInstall(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["shaders", instanceId] }),
  });
}

export function useSetActiveShader(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string | null) => api.shadersSetActive(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["shaders", instanceId] }),
  });
}

export function useRemoveShader(instanceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (projectId: string) => api.shadersRemove(instanceId, projectId),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["shaders", instanceId] }),
  });
}
