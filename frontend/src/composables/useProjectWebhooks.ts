import { shallowRef } from "vue";
import { apiCreateWebhook, apiDeleteWebhook, apiListWebhooks } from "@/lib/api/webhooks";
import type { WebhookInput, WebhookSummary } from "@/lib/types";

export function useProjectWebhooks() {
  const data = shallowRef<WebhookSummary[]>([]);
  const error = shallowRef<string | null>(null);
  const loading = shallowRef(false);
  const submitting = shallowRef(false);

  async function load(projectId: string) {
    loading.value = true;
    error.value = null;
    const result = await apiListWebhooks(projectId);
    if (result.success) data.value = result.data;
    else error.value = result.error ?? "Could not load webhooks";
    loading.value = false;
  }

  async function create(projectId: string, input: WebhookInput) {
    submitting.value = true;
    error.value = null;
    const result = await apiCreateWebhook(projectId, input);
    if (result.success) data.value = [result.data, ...data.value];
    else error.value = result.error ?? "Could not create webhook";
    submitting.value = false;
    return result.success;
  }

  async function remove(webhookId: string, confirmName: string) {
    const webhook = data.value.find((item) => item.id === webhookId);
    if (!webhook || confirmName !== webhook.name) return;
    submitting.value = true;
    error.value = null;
    const result = await apiDeleteWebhook(webhookId, confirmName);
    if (result.success) data.value = data.value.filter((webhook) => webhook.id !== webhookId);
    else error.value = result.error ?? "Could not remove webhook";
    submitting.value = false;
  }

  return { data, error, loading, submitting, load, create, remove };
}
