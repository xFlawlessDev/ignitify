import type { RuntimeContainerDetails, RuntimeContainerLogs } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

function containerPath(containerId: string) {
  return `/runtime/containers/${encodeURIComponent(containerId)}`;
}

function runtimePath(path: string, destination?: string) {
  if (!destination || destination === "local") return path;
  return `${path}?destination=${encodeURIComponent(destination)}`;
}

export function apiGetRuntimeContainerDetails(
  containerId: string,
  destination?: string,
): Promise<ApiResult<RuntimeContainerDetails>> {
  return apiFetch<RuntimeContainerDetails>(
    runtimePath(`${containerPath(containerId)}/details`, destination),
  );
}

export function apiGetRuntimeContainerLogs(
  containerId: string,
  destination?: string,
): Promise<ApiResult<RuntimeContainerLogs>> {
  return apiFetch<RuntimeContainerLogs>(
    runtimePath(`${containerPath(containerId)}/logs`, destination),
  );
}

export function apiRemoveRuntimeContainer(
  containerId: string,
  destination?: string,
): Promise<ApiResult<void>> {
  return apiFetch<void>(runtimePath(containerPath(containerId), destination), { method: "DELETE" });
}

export function apiUploadRuntimeContainerFile(
  containerId: string,
  file: File,
  fileDestination: string,
  destination?: string,
): Promise<ApiResult<void>> {
  const body = new FormData();
  body.append("destination", fileDestination);
  body.append("file", file);
  const path = runtimePath(`${containerPath(containerId)}/upload`, destination);
  return apiFetch<void>(path, {
    method: "POST",
    body,
  });
}
