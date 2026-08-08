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

export interface ServerSettingsResponse {
  server_domain: string;
  https_enabled: boolean;
  automatically_provision_ssl: boolean;
  certificate_provider: ServerCertificateProvider;
  custom_certificate_id: string | null;
  concurrent_builds: number;
  certificates: ServerCertificateSummary[];
  updated_at: string;
}

export interface ServerSettingsInput {
  server_domain: string;
  https_enabled: boolean;
  automatically_provision_ssl: boolean;
  certificate_provider: ServerCertificateProvider;
  custom_certificate_id: string | null;
  concurrent_builds: number;
}

const endpoint = "/settings/server";

export function apiGetServerSettings(): Promise<ApiResult<ServerSettingsResponse>> {
  return apiFetch<ServerSettingsResponse>(endpoint);
}

export function apiUpdateServerSettings(
  input: ServerSettingsInput,
): Promise<ApiResult<ServerSettingsResponse>> {
  return apiFetch<ServerSettingsResponse>(endpoint, {
    method: "PATCH",
    body: JSON.stringify(input),
  });
}

export function apiCreateServerCertificate(
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

export function apiDeleteServerCertificate(certificateId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/certificates/${encodeURIComponent(certificateId)}`, {
    method: "DELETE",
  });
}
