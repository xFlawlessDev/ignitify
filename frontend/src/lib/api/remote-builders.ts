import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export interface RemoteBuilderSummary {
  id: string;
  name: string;
  endpoint: string;
  registry_repository: string;
  tls_server_name: string | null;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface RemoteBuilderInput {
  name: string;
  endpoint: string;
  registry_repository: string;
  tls_server_name?: string | null;
  ca_certificate: string;
  client_certificate: string;
  client_key: string;
  is_default: boolean;
}

const endpoint = "/remote-builders";

export function apiListRemoteBuilders(): Promise<ApiResult<RemoteBuilderSummary[]>> {
  return apiFetch<RemoteBuilderSummary[]>(endpoint);
}

export function apiCreateRemoteBuilder(
  input: RemoteBuilderInput,
): Promise<ApiResult<RemoteBuilderSummary>> {
  return apiFetch<RemoteBuilderSummary>(endpoint, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiUpdateRemoteBuilder(
  builderId: string,
  input: RemoteBuilderInput,
): Promise<ApiResult<RemoteBuilderSummary>> {
  return apiFetch<RemoteBuilderSummary>(`${endpoint}/${encodeURIComponent(builderId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiDeleteRemoteBuilder(builderId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/${encodeURIComponent(builderId)}`, {
    method: "DELETE",
  });
}

export function apiSetDefaultRemoteBuilder(
  builderId: string,
): Promise<ApiResult<RemoteBuilderSummary>> {
  return apiFetch<RemoteBuilderSummary>(`${endpoint}/${encodeURIComponent(builderId)}/default`, {
    method: "POST",
  });
}
