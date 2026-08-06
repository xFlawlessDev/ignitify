import type {
  ProjectEnvironmentResponse,
  ProjectEnvironmentVariableInput,
  ProjectInput,
  ProjectSummary,
} from "../types";
import { apiFetch } from "./core";
import type { ApiResult } from "./core";

export function apiListProjects(): Promise<ApiResult<ProjectSummary[]>> {
  return apiFetch<ProjectSummary[]>("/projects");
}

export function apiCreateProject(input: ProjectInput): Promise<ApiResult<ProjectSummary>> {
  return apiFetch<ProjectSummary>("/projects", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiGetProject(projectId: string): Promise<ApiResult<ProjectSummary>> {
  return apiFetch<ProjectSummary>(`/projects/${encodeURIComponent(projectId)}`);
}

export function apiUpdateProject(
  projectId: string,
  input: ProjectInput,
): Promise<ApiResult<ProjectSummary>> {
  return apiFetch<ProjectSummary>(`/projects/${encodeURIComponent(projectId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiGetProjectEnvironment(
  projectId: string,
): Promise<ApiResult<ProjectEnvironmentResponse>> {
  return apiFetch<ProjectEnvironmentResponse>(
    `/projects/${encodeURIComponent(projectId)}/environment`,
  );
}

export function apiUpdateProjectEnvironment(
  projectId: string,
  variables: ProjectEnvironmentVariableInput[],
): Promise<ApiResult<ProjectEnvironmentResponse>> {
  return apiFetch<ProjectEnvironmentResponse>(
    `/projects/${encodeURIComponent(projectId)}/environment`,
    {
      method: "PUT",
      body: JSON.stringify({ variables }),
    },
  );
}
