import type { DashboardSummary, RuntimeStatus, TerminalCapability } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiGetDashboard(): Promise<ApiResult<DashboardSummary>> {
  return apiFetch<DashboardSummary>("/dashboard");
}

export function apiGetRuntimeStatus(): Promise<ApiResult<RuntimeStatus>> {
  return apiFetch<RuntimeStatus>("/runtime/status");
}

export function apiGetTerminalCapability(
  serviceId: string,
): Promise<ApiResult<TerminalCapability>> {
  return apiFetch<TerminalCapability>(
    `/services/${encodeURIComponent(serviceId)}/terminal/capability`,
  );
}
