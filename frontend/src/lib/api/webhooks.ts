import type { WebhookInput, WebhookSummary } from "../types";
import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export function apiListWebhooks(projectId: string): Promise<ApiResult<WebhookSummary[]>> {
  return apiFetch<WebhookSummary[]>(`/projects/${encodeURIComponent(projectId)}/webhooks`);
}

export function apiCreateWebhook(
  projectId: string,
  input: WebhookInput,
): Promise<ApiResult<WebhookSummary>> {
  return apiFetch<WebhookSummary>(`/projects/${encodeURIComponent(projectId)}/webhooks`, {
    method: "POST",
    body: JSON.stringify(input),
  });
}

export function apiDeleteWebhook(
  webhookId: string,
  confirmName: string,
): Promise<ApiResult<WebhookSummary>> {
  return apiFetch<WebhookSummary>(`/webhooks/${encodeURIComponent(webhookId)}`, {
    method: "DELETE",
    body: JSON.stringify({ confirm_name: confirmName }),
  });
}
