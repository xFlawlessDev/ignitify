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
  if (input.kind === "compose") {
    const composeYaml = input.compose_yaml ?? "";
    if (!composeYaml.trim()) {
      return Promise.resolve({
        success: false,
        data: null as unknown as ServiceSummary,
        error: "Compose YAML is required before saving the service.",
      });
    }
    if (composeYaml.length > 1024 * 1024 || composeYaml.includes(String.fromCharCode(0))) {
      return Promise.resolve({
        success: false,
        data: null as unknown as ServiceSummary,
        error: "Compose YAML must be at most 1 MiB and cannot contain NUL characters.",
      });
    }
  }
  return apiFetch<ServiceSummary>(`/services/${encodeURIComponent(serviceId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}
