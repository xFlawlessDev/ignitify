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
  agent: RemoteServerAgentSummary | null;
  is_default: boolean;
  created_at: string;
  updated_at: string;
}

export interface RemoteServerAgentSummary {
  status: "pending" | "online" | "offline";
  version: string | null;
  cpu_usage_percentage: number | null;
  cpu_cores: number | null;
  memory_used_bytes: number | null;
  memory_total_bytes: number | null;
  disk_used_bytes: number | null;
  disk_total_bytes: number | null;
  docker_containers: number | null;
  docker_running_containers: number | null;
  last_heartbeat_at: string | null;
  last_error: string | null;
  installed_at: string;
  updated_at: string;
}

export interface RemoteServerAgentInstallResult {
  agent: RemoteServerAgentSummary;
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
  return apiFetch<RemoteServerCheckResult>(`${endpoint}/${encodeURIComponent(serverId)}/check`, {
    method: "POST",
  });
}

export function apiInstallRemoteServerAgent(
  serverId: string,
): Promise<ApiResult<RemoteServerAgentInstallResult>> {
  return apiFetch<RemoteServerAgentInstallResult>(
    `${endpoint}/${encodeURIComponent(serverId)}/agent/install`,
    { method: "POST" },
  );
}

export function apiGetRemoteServerAgent(
  serverId: string,
): Promise<ApiResult<RemoteServerAgentSummary | null>> {
  return apiFetch<RemoteServerAgentSummary | null>(
    `${endpoint}/${encodeURIComponent(serverId)}/agent`,
  );
}
