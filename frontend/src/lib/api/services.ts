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
  const validationError = validateComposeInput(input);
  if (validationError) return rejectedServiceInput(validationError);
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
  const validationError = validateComposeInput(input);
  if (validationError) return rejectedServiceInput(validationError);
  return apiFetch<ServiceSummary>(`/services/${encodeURIComponent(serviceId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

function validateComposeInput(input: ServiceInput): string | null {
  if (input.kind !== "compose") return null;

  const composeYaml = input.compose_yaml ?? "";
  const isGitComposeSource =
    input.source_config?.source === "compose" && Boolean(input.source_config.provider_id?.trim());
  if (!isGitComposeSource && !composeYaml.trim()) {
    return "Compose YAML is required before saving the service.";
  }
  if (composeYaml.length > 1024 * 1024 || composeYaml.includes(String.fromCharCode(0))) {
    return "Compose YAML must be at most 1 MiB and cannot contain NUL characters.";
  }
  return null;
}

function rejectedServiceInput(error: string): Promise<ApiResult<ServiceSummary>> {
  return Promise.resolve({
    success: false,
    data: null as unknown as ServiceSummary,
    error,
  });
}

export function apiDeleteService(serviceId: string, confirmName: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`/services/${encodeURIComponent(serviceId)}`, {
    method: "DELETE",
    body: JSON.stringify({ confirm_name: confirmName }),
  });
}
