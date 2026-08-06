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

export function apiGetRuntimeStatus(): Promise<ApiResult<RuntimeStatus>> {
  return apiFetch<RuntimeStatus>("/runtime/status");
}

export function apiGetRuntimeContainers(): Promise<ApiResult<RuntimeContainerInventory>> {
  return apiFetch<RuntimeContainerInventory>("/runtime/containers");
}

export function apiGetSystemMetrics(): Promise<ApiResult<SystemMetrics>> {
  return apiFetch<SystemMetrics>("/runtime/metrics");
}
