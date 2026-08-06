<script setup lang="ts">
import { ArrowUpRight, GitBranch, GitPullRequest, PlugZap, Server, Trash2 } from "@lucide/vue";
import type { ProviderSummary } from "@/lib/types";

defineProps<{
  providers: ProviderSummary[];
  busy?: boolean;
  canManage?: boolean;
  testingId?: string | null;
}>();

const emit = defineEmits<{
  remove: [provider: ProviderSummary];
  test: [provider: ProviderSummary];
}>();

const providerLabels: Record<ProviderSummary["kind"], string> = {
  git: "Generic Git",
  gitea: "Gitea",
  gitlab: "GitLab",
  github: "GitHub",
};

const authLabels: Record<ProviderSummary["auth_mode"], string> = {
  token: "Access token",
  oauth: "OAuth app",
  github_app: "GitHub App",
};

function formatDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}
</script>

<template>
  <div class="overflow-hidden border border-border bg-card">
    <div
      v-for="provider in providers"
      :key="provider.id"
      class="grid gap-4 border-b border-border px-4 py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:px-5"
    >
      <div class="flex min-w-0 items-start gap-3">
        <span class="grid size-9 shrink-0 place-items-center border border-border bg-muted">
          <GitBranch class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </span>
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
            <strong class="truncate text-[13px] font-medium">{{ provider.name }}</strong>
            <span class="font-mono text-[10px] tracking-[0.08em] text-muted-foreground uppercase">
              {{ providerLabels[provider.kind] }}
            </span>
            <span class="font-mono text-[10px] tracking-[0.08em] text-muted-foreground uppercase">
              {{ authLabels[provider.auth_mode] }}
            </span>
          </div>
          <a
            class="mt-1 inline-flex max-w-full items-center gap-1 truncate text-xs text-muted-foreground underline decoration-border underline-offset-2 transition-colors hover:text-foreground"
            :href="provider.base_url"
            target="_blank"
            rel="noreferrer"
          >
            <span class="truncate">{{ provider.base_url }}</span>
            <ArrowUpRight class="size-3 shrink-0" :stroke-width="1.5" />
          </a>
          <p
            class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] text-muted-foreground"
          >
            <span class="inline-flex items-center gap-1.5">
              <span class="status-dot" data-status="healthy" />
              {{ provider.token_configured ? "Token configured" : "Token missing" }}
            </span>
            <span v-if="provider.username" class="inline-flex items-center gap-1.5">
              <GitPullRequest class="size-3" :stroke-width="1.5" />
              {{ provider.username }}
            </span>
            <span class="inline-flex items-center gap-1.5">
              <Server class="size-3" :stroke-width="1.5" />
              Added {{ formatDate(provider.created_at) }}
            </span>
            <span v-if="provider.last_verified_at" class="inline-flex items-center gap-1.5">
              Tested {{ formatDate(provider.last_verified_at) }}
            </span>
          </p>
        </div>
      </div>
      <div v-if="canManage" class="flex items-center gap-1 justify-self-end">
        <button
          class="grid size-8 place-items-center rounded-[3px] text-muted-foreground transition-colors hover:bg-primary/10 hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
          type="button"
          :disabled="busy || testingId !== null"
          aria-label="Test provider connection"
          title="Test provider connection"
          @click="emit('test', provider)"
        >
          <PlugZap
            class="size-4"
            :class="testingId === provider.id ? 'animate-pulse' : ''"
            :stroke-width="1.5"
          />
        </button>
        <button
          class="grid size-8 place-items-center rounded-[3px] text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive disabled:pointer-events-none disabled:opacity-50"
          type="button"
          :disabled="busy || testingId !== null"
          aria-label="Remove provider"
          title="Remove provider"
          @click="emit('remove', provider)"
        >
          <Trash2 class="size-4" :stroke-width="1.5" />
        </button>
      </div>
    </div>
  </div>
</template>
