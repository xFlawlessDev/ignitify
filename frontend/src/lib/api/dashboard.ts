import type {
  DashboardSummary,
  RuntimeContainerInventory,
  RuntimeStatus,
  SystemMetrics,
} from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiGetDashboard(): Promise<ApiResult<DashboardSummary>> {
  return apiFetch<DashboardSummary>("/dashboard");
}

export function apiGetRuntimeStatus(destination?: string): Promise<ApiResult<RuntimeStatus>> {
  return apiFetch<RuntimeStatus>(runtimePath("/runtime/status", destination));
}

export function apiGetRuntimeContainers(
  destination?: string,
): Promise<ApiResult<RuntimeContainerInventory>> {
  return apiFetch<RuntimeContainerInventory>(runtimePath("/runtime/containers", destination));
}

export function apiGetSystemMetrics(): Promise<ApiResult<SystemMetrics>> {
  return apiFetch<SystemMetrics>("/runtime/metrics");
}

function runtimePath(path: string, destination?: string): string {
  if (!destination || destination === "local") return path;
  return `${path}?destination=${encodeURIComponent(destination)}`;
}
