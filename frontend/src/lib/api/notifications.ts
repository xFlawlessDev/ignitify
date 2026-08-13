import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export type NotificationChannelKind = "telegram" | "discord" | "smtp" | "resend" | "webhook";

export type NotificationEventKind =
  | "deployment.queued"
  | "deployment.preparing"
  | "deployment.running"
  | "deployment.healthy"
  | "deployment.failed"
  | "deployment.stopping"
  | "deployment.stopped"
  | "deployment.superseded"
  | "backup.succeeded"
  | "backup.failed"
  | "remote_agent.offline"
  | "remote_server.authentication_failed";

export interface NotificationChannel {
  id: string;
  name: string;
  kind: NotificationChannelKind;
  enabled: boolean;
  event_types: NotificationEventKind[];
  configuration_summary: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface NotificationChannelInput {
  name: string;
  kind: NotificationChannelKind;
  enabled: boolean;
  event_types: NotificationEventKind[];
  configuration: Record<string, string | number | boolean | null>;
}

const endpoint = "/notifications";

export function apiListNotificationChannels(): Promise<ApiResult<NotificationChannel[]>> {
  return apiFetch<NotificationChannel[]>(endpoint);
}

export function apiCreateNotificationChannel(
  input: NotificationChannelInput,
): Promise<ApiResult<NotificationChannel>> {
  return apiFetch<NotificationChannel>(endpoint, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiUpdateNotificationChannel(
  channelId: string,
  input: NotificationChannelInput,
): Promise<ApiResult<NotificationChannel>> {
  return apiFetch<NotificationChannel>(`${endpoint}/${encodeURIComponent(channelId)}`, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export function apiDeleteNotificationChannel(channelId: string): Promise<ApiResult<void>> {
  return apiFetch<void>(`${endpoint}/${encodeURIComponent(channelId)}`, {
    method: "DELETE",
  });
}
