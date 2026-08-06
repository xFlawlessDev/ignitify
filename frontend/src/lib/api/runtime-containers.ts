import type { RuntimeContainerDetails, RuntimeContainerLogs } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

function containerPath(containerId: string) {
  return `/runtime/containers/${encodeURIComponent(containerId)}`;
}

export function apiGetRuntimeContainerDetails(
  containerId: string,
): Promise<ApiResult<RuntimeContainerDetails>> {
  return apiFetch<RuntimeContainerDetails>(`${containerPath(containerId)}/details`);
}

export function apiGetRuntimeContainerLogs(
  containerId: string,
): Promise<ApiResult<RuntimeContainerLogs>> {
  return apiFetch<RuntimeContainerLogs>(`${containerPath(containerId)}/logs`);
}

export function apiRemoveRuntimeContainer(containerId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(containerPath(containerId), { method: "DELETE" });
}

export function apiUploadRuntimeContainerFile(
  containerId: string,
  file: File,
  destination: string,
): Promise<ApiResult<void>> {
  const body = new FormData();
  body.append("destination", destination);
  body.append("file", file);
  return apiFetch<void>(`${containerPath(containerId)}/upload`, {
    method: "POST",
    body,
  });
}
