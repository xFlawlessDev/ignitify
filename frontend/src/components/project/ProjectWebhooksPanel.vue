<script setup lang="ts">
import { GitBranch, Plus, Trash2 } from "@lucide/vue";
import { shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { WebhookSummary } from "@/lib/types";

const props = defineProps<{
  canManage: boolean;
  error: string | null;
  loading: boolean;
  submitting: boolean;
  webhooks: WebhookSummary[];
}>();

const emit = defineEmits<{
  create: [input: { name: string; url: string; secret?: string }];
  remove: [webhook: WebhookSummary, confirmName: string];
  retry: [];
}>();

const name = shallowRef("");
const url = shallowRef("");
const secret = shallowRef("");
const confirmName = shallowRef<string | null>(null);
const confirmValue = shallowRef("");

function submit() {
  if (!name.value.trim() || !url.value.trim()) return;
  emit("create", {
    name: name.value.trim(),
    url: url.value.trim(),
    ...(secret.value ? { secret: secret.value } : {}),
  });
  name.value = "";
  url.value = "";
  secret.value = "";
}

function startRemove(webhook: WebhookSummary) {
  confirmName.value = webhook.name;
  confirmValue.value = "";
}

function confirmRemove(webhook: WebhookSummary) {
  if (confirmValue.value !== webhook.name) return;
  emit("remove", webhook, confirmValue.value);
  confirmName.value = null;
  confirmValue.value = "";
}
</script>

<template>
  <section class="mt-[22px] border border-border bg-card">
    <div class="flex items-center justify-between border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Delivery</p>
        <h2 class="mt-2 text-base font-medium">Webhooks</h2>
      </div>
      <GitBranch class="size-4 text-muted-foreground" :stroke-width="1.5" />
    </div>
    <div v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" role="status">
      Loading webhooks...
    </div>
    <div v-else-if="error" class="px-5 py-8 text-sm text-destructive" role="alert">
      {{ error }}
      <Button class="ml-3" size="sm" variant="outline" @click="$emit('retry')">Retry</Button>
    </div>
    <template v-else>
      <div
        v-if="canManage"
        class="grid gap-3 border-b border-border p-5 md:grid-cols-[1fr_1.5fr_1fr_auto]"
      >
        <Input v-model="name" placeholder="Name" aria-label="Webhook name" />
        <Input
          v-model="url"
          type="url"
          placeholder="https://example.com/hook"
          aria-label="Webhook URL"
        />
        <Input
          v-model="secret"
          type="password"
          placeholder="Signing secret"
          aria-label="Webhook secret"
        />
        <Button :disabled="submitting || !name.trim() || !url.trim()" type="button" @click="submit">
          <Plus class="size-4" :stroke-width="1.5" /> Add
        </Button>
      </div>
      <div v-if="!webhooks.length" class="px-5 py-8 text-sm text-muted-foreground">
        No webhooks configured.
      </div>
      <div v-else class="divide-y divide-border">
        <div
          v-for="webhook in webhooks"
          :key="webhook.id"
          class="flex items-center justify-between gap-4 px-5 py-4"
        >
          <div class="min-w-0">
            <p class="text-sm font-medium">{{ webhook.name }}</p>
            <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground">
              {{ webhook.url }}
            </p>
            <p class="mt-1 text-[11px] text-muted-foreground">
              {{ webhook.secret_configured ? "Secret configured" : "No signing secret" }} ·
              {{ webhook.is_enabled ? "Enabled" : "Disabled" }}
            </p>
          </div>
          <Button
            v-if="canManage"
            size="icon"
            variant="ghost"
            :disabled="submitting"
            aria-label="Remove webhook"
            @click="startRemove(webhook)"
          >
            <Trash2 class="size-4" :stroke-width="1.5" />
          </Button>
          <div
            v-if="confirmName === webhook.name"
            class="mt-3 flex w-full items-center gap-2 border-t border-border pt-3"
          >
            <Input
              v-model="confirmValue"
              :placeholder="`Type ${webhook.name} to remove`"
              :aria-label="`Confirm removal of ${webhook.name}`"
            />
            <Button
              size="sm"
              variant="destructive"
              :disabled="submitting || confirmValue !== webhook.name"
              @click="confirmRemove(webhook)"
            >
              Remove
            </Button>
          </div>
        </div>
      </div>
    </template>
  </section>
</template>
