import type { RegistryInput, RegistrySummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiListRegistries(): Promise<ApiResult<RegistrySummary[]>> {
  return apiFetch<RegistrySummary[]>("/registries");
}

export function apiCreateRegistry(input: RegistryInput): Promise<ApiResult<RegistrySummary>> {
  return apiFetch<RegistrySummary>("/registries", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiDeleteRegistry(
  registryId: string,
  confirmName: string,
): Promise<ApiResult<RegistrySummary>> {
  return apiFetch<RegistrySummary>(`/registries/${encodeURIComponent(registryId)}`, {
    method: "DELETE",
    body: JSON.stringify({ confirm_name: confirmName }),
  });
}
