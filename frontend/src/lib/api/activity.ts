import type { ActivitySummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiListProjectActivity(
  projectId: string,
  options: { before?: string; limit?: number } = {},
): Promise<ApiResult<ActivitySummary[]>> {
  const search = new URLSearchParams();
  if (options.before) search.set("before", options.before);
  if (options.limit) search.set("limit", String(options.limit));
  const query = search.size ? `?${search}` : "";
  return apiFetch<ActivitySummary[]>(`/projects/${encodeURIComponent(projectId)}/activity${query}`);
}
