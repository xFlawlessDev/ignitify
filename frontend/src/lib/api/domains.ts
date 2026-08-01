import type { DomainSummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiListDomains(serviceId: string): Promise<ApiResult<DomainSummary[]>> {
  return apiFetch<DomainSummary[]>(`/services/${encodeURIComponent(serviceId)}/domains`);
}

export function apiCreateDomain(
  serviceId: string,
  hostname: string,
): Promise<ApiResult<DomainSummary>> {
  return apiFetch<DomainSummary>(`/services/${encodeURIComponent(serviceId)}/domains`, {
    method: "POST",
    body: JSON.stringify({ hostname }),
  });
}

export function apiRemoveDomain(
  domainId: string,
  confirmHostname: string,
): Promise<ApiResult<DomainSummary>> {
  return apiFetch<DomainSummary>(`/domains/${encodeURIComponent(domainId)}`, {
    method: "DELETE",
    body: JSON.stringify({ confirm_hostname: confirmHostname }),
  });
}
