import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export interface RemoteServerSummary {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  deploy_path: string;
  private_key_configured: boolean;
  public_key_configured: boolean;
  known_hosts_configured: boolean;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface RemoteServerInput {
  name: string;
  host: string;
  port: number;
  username: string;
  deploy_path: string;
  private_key?: string;
  public_key?: string;
  known_hosts?: string;
  is_default: boolean;
}

export interface RemoteServerCheckResult {
  connected: boolean;
  latency_ms: number;
}

const endpoint = "/remote-servers";

export function apiListRemoteServers(): Promise<ApiResult<RemoteServerSummary[]>> {
  return apiFetch<RemoteServerSummary[]>(endpoint);
}

export function apiCreateRemoteServer(
  input: RemoteServerInput,
): Promise<ApiResult<RemoteServerSummary>> {
  return apiFetch<RemoteServerSummary>(endpoint, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiUpdateRemoteServer(
  serverId: string,
  input: RemoteServerInput,
): Promise<ApiResult<RemoteServerSummary>> {
  return apiFetch<RemoteServerSummary>(`${endpoint}/${encodeURIComponent(serverId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiDeleteRemoteServer(serverId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/${encodeURIComponent(serverId)}`, {
    method: "DELETE",
  });
}

export function apiSetDefaultRemoteServer(
  serverId: string,
): Promise<ApiResult<RemoteServerSummary>> {
  return apiFetch<RemoteServerSummary>(`${endpoint}/${encodeURIComponent(serverId)}/default`, {
    method: "POST",
  });
}

export function apiCheckRemoteServer(
  serverId: string,
): Promise<ApiResult<RemoteServerCheckResult>> {
  return apiFetch<RemoteServerCheckResult>(
    `${endpoint}/${encodeURIComponent(serverId)}/check`,
    { method: "POST" },
  );
}
