import type { OperationalHealthSummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiGetOperationalHealthSummary(): Promise<ApiResult<OperationalHealthSummary>> {
  return apiFetch<OperationalHealthSummary>("/operations/health-summary");
}
