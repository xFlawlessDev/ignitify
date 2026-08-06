import type {
  GithubManifestInput,
  GithubManifestStart,
  ProviderInput,
  ProviderSummary,
} from "../types";
import { apiFetch } from "./core";
import type { ApiResult } from "./core";

export function apiListProviders(): Promise<ApiResult<ProviderSummary[]>> {
  return apiFetch<ProviderSummary[]>("/providers");
}

export function apiCreateProvider(input: ProviderInput): Promise<ApiResult<ProviderSummary>> {
  return apiFetch<ProviderSummary>("/providers", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiStartGithubAppManifest(
  input: GithubManifestInput,
): Promise<ApiResult<GithubManifestStart>> {
  return apiFetch<GithubManifestStart>("/providers/github/manifest", {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiUpdateProvider(
  providerId: string,
  input: ProviderInput,
): Promise<ApiResult<ProviderSummary>> {
  return apiFetch<ProviderSummary>(`/providers/${encodeURIComponent(providerId)}`, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiDeleteProvider(providerId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`/providers/${encodeURIComponent(providerId)}`, {
    method: "DELETE",
  });
}
