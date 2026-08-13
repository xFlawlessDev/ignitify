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
  | "remote_server.authentication_failed"
  | "operations.alert";

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

export interface NotificationDelivery {
  id: string;
  channel_id: string;
  channel_name: string;
  channel_kind: NotificationChannelKind;
  source_kind: string;
  source_id: string;
  event_kind: NotificationEventKind;
  status: "running" | "succeeded" | "failed";
  attempt_count: number;
  created_at: string;
  completed_at: string | null;
  message: string | null;
}

const endpoint = "/notifications";

export function apiListNotificationChannels(): Promise<ApiResult<NotificationChannel[]>> {
  return apiFetch<NotificationChannel[]>(endpoint);
}

export function apiListNotificationDeliveries(
  limit = 50,
): Promise<ApiResult<NotificationDelivery[]>> {
  return apiFetch<NotificationDelivery[]>(`${endpoint}/deliveries?limit=${limit}`);
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
