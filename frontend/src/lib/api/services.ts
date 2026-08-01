import type { ServiceInput, ServiceSummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiListServices(projectId: string): Promise<ApiResult<ServiceSummary[]>> {
  return apiFetch<ServiceSummary[]>(`/projects/${encodeURIComponent(projectId)}/services`);
}

export function apiCreateService(
  projectId: string,
  input: ServiceInput,
): Promise<ApiResult<ServiceSummary>> {
  return apiFetch<ServiceSummary>(`/projects/${encodeURIComponent(projectId)}/services`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiGetService(serviceId: string): Promise<ApiResult<ServiceSummary>> {
  return apiFetch<ServiceSummary>(`/services/${encodeURIComponent(serviceId)}`);
}

export function apiUpdateService(
  serviceId: string,
  input: ServiceInput,
): Promise<ApiResult<ServiceSummary>> {
  return apiFetch<ServiceSummary>(`/services/${encodeURIComponent(serviceId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}
