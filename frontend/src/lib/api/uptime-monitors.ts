import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export type UptimeMonitorKind = "http" | "tcp";
export type UptimeMonitorStatus = "pending" | "up" | "down";

export interface UptimeMonitorSummary {
  id: string;
  name: string;
  target: string;
  kind: UptimeMonitorKind;
  interval_seconds: number;
  enabled: boolean;
  status: UptimeMonitorStatus;
  history: Array<UptimeMonitorStatus | "unknown">;
  latency_ms: number | null;
  last_checked_at: string | null;
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface UptimeMonitorInput {
  name: string;
  target: string;
  kind: UptimeMonitorKind;
  interval_seconds: number;
  enabled: boolean;
}

const endpoint = "/uptime-monitors";

export function apiListUptimeMonitors(): Promise<ApiResult<UptimeMonitorSummary[]>> {
  return apiFetch<UptimeMonitorSummary[]>(endpoint);
}

export function apiCreateUptimeMonitor(
  input: UptimeMonitorInput,
): Promise<ApiResult<UptimeMonitorSummary>> {
  return apiFetch<UptimeMonitorSummary>(endpoint, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiUpdateUptimeMonitor(
  monitorId: string,
  input: UptimeMonitorInput,
): Promise<ApiResult<UptimeMonitorSummary>> {
  return apiFetch<UptimeMonitorSummary>(`${endpoint}/${encodeURIComponent(monitorId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiDeleteUptimeMonitor(monitorId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/${encodeURIComponent(monitorId)}`, {
    method: "DELETE",
  });
}
