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

export type UptimeAvailabilityStatus = "healthy" | "warning" | "exhausted" | "insufficient_data";

export interface UptimeCheckHistoryEntry {
  status: Exclude<UptimeMonitorStatus, "pending">;
  latency_ms: number | null;
  error: string | null;
  checked_at: string;
}

export interface UptimeMonitorHistorySummary {
  window_hours: number;
  total_checks: number;
  successful_checks: number;
  failed_checks: number;
  availability_percentage: number | null;
  error_budget_percentage: number | null;
  budget_consumed_percentage: number | null;
  status: UptimeAvailabilityStatus;
}

export interface UptimeMonitorHistory {
  monitor_id: string;
  retention_days: number;
  checks: UptimeCheckHistoryEntry[];
  summary: UptimeMonitorHistorySummary;
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

export function apiGetUptimeMonitorHistory(
  monitorId: string,
  options: { hours?: number; limit?: number } = {},
): Promise<ApiResult<UptimeMonitorHistory>> {
  const params = new URLSearchParams();
  if (options.hours !== undefined) params.set("hours", String(options.hours));
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  const query = params.size > 0 ? `?${params.toString()}` : "";
  return apiFetch<UptimeMonitorHistory>(
    `${endpoint}/${encodeURIComponent(monitorId)}/history${query}`,
  );
}
