import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export type S3ServerSideEncryption = "AES256" | "provider-default";

export interface BackupS3Destination {
  endpoint: string;
  region: string;
  bucket: string;
  prefix: string;
  server_side_encryption: S3ServerSideEncryption;
  created_at: string;
  updated_at: string;
}

export interface BackupS3DestinationInput {
  endpoint: string;
  region: string;
  bucket: string;
  prefix: string;
  access_key_id: string;
  secret_access_key: string;
  session_token?: string;
  server_side_encryption: S3ServerSideEncryption;
}

const endpoint = "/settings/backup-destination/s3";

export function apiGetBackupS3Destination(): Promise<ApiResult<BackupS3Destination | null>> {
  return apiFetch<BackupS3Destination | null>(endpoint);
}

export function apiUpdateBackupS3Destination(
  input: BackupS3DestinationInput,
): Promise<ApiResult<BackupS3Destination>> {
  return apiFetch<BackupS3Destination>(endpoint, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export function apiDeleteBackupS3Destination(): Promise<ApiResult<void>> {
  return apiFetch<void>(endpoint, { method: "DELETE" });
}
