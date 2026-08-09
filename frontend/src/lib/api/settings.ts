import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export type ServerCertificateProvider = "none" | "lets-encrypt" | "custom";

export interface ServerCertificateSummary {
  id: string;
  name: string;
  certificate_file_name: string;
  private_key_file_name: string;
  created_at: string;
  updated_at: string;
}

export interface InfrastructureHealthStatus {
  database: "ready" | "unavailable";
  runtime: "ready" | "unavailable";
  worker: "ready" | "unavailable";
  ingress: "ready" | "unavailable";
}

export interface ApplicationEnvironmentStatus {
  public_origin: string;
  secure_cookies: boolean;
}

export interface InfrastructureSettingsResponse {
  application: ApplicationEnvironmentStatus;
  application_domain_suffix: string;
  https_enabled: boolean;
  automatically_provision_ssl: boolean;
  acme_email: string;
  dns_record_type: "a" | "cname";
  dns_record_target: string;
  fallback_page_heading: string;
  fallback_page_message: string;
  certificate_provider: ServerCertificateProvider;
  custom_certificate_id: string | null;
  certificates: ServerCertificateSummary[];
  health: InfrastructureHealthStatus;
  updated_at: string;
}

export interface InfrastructureSettingsInput {
  application_domain_suffix: string;
  https_enabled: boolean;
  automatically_provision_ssl: boolean;
  acme_email: string;
  dns_record_type: "a" | "cname";
  dns_record_target: string;
  fallback_page_heading: string;
  fallback_page_message: string;
  certificate_provider: ServerCertificateProvider;
  custom_certificate_id: string | null;
}

const endpoint = "/settings/infrastructure";

export function apiGetInfrastructureSettings(): Promise<ApiResult<InfrastructureSettingsResponse>> {
  return apiFetch<InfrastructureSettingsResponse>(endpoint);
}

export function apiUpdateInfrastructureSettings(
  input: InfrastructureSettingsInput,
): Promise<ApiResult<InfrastructureSettingsResponse>> {
  return apiFetch<InfrastructureSettingsResponse>(endpoint, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiCreateInfrastructureCertificate(
  name: string,
  certificateFile: File,
  privateKeyFile: File,
): Promise<ApiResult<ServerCertificateSummary>> {
  const body = new FormData();
  body.append("name", name);
  body.append("certificate", certificateFile, certificateFile.name);
  body.append("private_key", privateKeyFile, privateKeyFile.name);
  return apiFetch<ServerCertificateSummary>(`${endpoint}/certificates`, {
    method: "POST",
    body,
  });
}

export function apiDeleteInfrastructureCertificate(
  certificateId: string,
): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/certificates/${encodeURIComponent(certificateId)}`, {
    method: "DELETE",
  });
}
