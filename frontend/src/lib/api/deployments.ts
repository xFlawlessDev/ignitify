import type { DeploymentSummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiDeployService(
  serviceId: string,
  idempotencyKey: string,
): Promise<ApiResult<DeploymentSummary>> {
  return apiFetch<DeploymentSummary>(`/services/${encodeURIComponent(serviceId)}/deployments`, {
    method: "POST",
    headers: { "Idempotency-Key": idempotencyKey },
  });
}

export function apiListDeployments(
  serviceId: string,
  before?: string,
): Promise<ApiResult<DeploymentSummary[]>> {
  const query = before ? `?before=${encodeURIComponent(before)}` : "";
  return apiFetch<DeploymentSummary[]>(
    `/services/${encodeURIComponent(serviceId)}/deployments${query}`,
  );
}

export function apiListProjectDeployments(
  projectId: string,
  before?: string,
): Promise<ApiResult<DeploymentSummary[]>> {
  const query = before ? `?before=${encodeURIComponent(before)}` : "";
  return apiFetch<DeploymentSummary[]>(
    `/projects/${encodeURIComponent(projectId)}/deployments${query}`,
  );
}

export function apiStopService(serviceId: string): Promise<ApiResult<DeploymentSummary>> {
  return apiFetch<DeploymentSummary>(`/services/${encodeURIComponent(serviceId)}/stop`, {
    method: "POST",
  });
}

export function apiGetDeployment(deploymentId: string): Promise<ApiResult<DeploymentSummary>> {
  return apiFetch<DeploymentSummary>(`/deployments/${encodeURIComponent(deploymentId)}`);
}

export function apiRollbackDeployment(
  deploymentId: string,
  idempotencyKey: string,
): Promise<ApiResult<DeploymentSummary>> {
  return apiFetch<DeploymentSummary>(`/deployments/${encodeURIComponent(deploymentId)}/rollback`, {
    method: "POST",
    headers: { "Idempotency-Key": idempotencyKey },
  });
}

export function apiCancelDeployment(deploymentId: string): Promise<ApiResult<DeploymentSummary>> {
  return apiFetch<DeploymentSummary>(`/deployments/${encodeURIComponent(deploymentId)}/cancel`, {
    method: "POST",
  });
}
