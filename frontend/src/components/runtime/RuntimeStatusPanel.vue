<script setup lang="ts">
import { CircleAlert, Server } from "@lucide/vue";
import { Skeleton } from "@/components/ui/skeleton";
import type { RuntimeStatus } from "@/lib/types";

withDefaults(
  defineProps<{
    runtime: RuntimeStatus | null;
    loading?: boolean;
  }>(),
  { loading: false },
);

function formatBytes(value: number) {
  if (!value) return "Unavailable";
  return `${(value / 1024 ** 3).toFixed(1)} GiB`;
}
</script>

<template>
  <section class="min-w-0 app-surface">
    <div class="app-panel-header px-5 py-4">
      <p class="ui-label">Runtime</p>
      <h2 class="mt-2 text-base font-medium">Host visibility</h2>
    </div>
    <div
      v-if="loading"
      class="grid gap-4 px-5 py-5"
      role="status"
      aria-label="Loading runtime status"
    >
      <div class="flex items-center gap-3">
        <Skeleton class="size-4" />
        <Skeleton class="h-3 w-28" />
      </div>
      <div class="grid gap-3">
        <div
          v-for="label in ['Database', 'Runtime', 'Worker']"
          :key="label"
          class="flex items-center justify-between gap-4"
        >
          <Skeleton class="h-2.5 w-16" />
          <Skeleton class="h-2.5 w-12" />
        </div>
      </div>
    </div>
    <div v-else-if="runtime" class="px-5 py-5">
      <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
      <p class="mt-4 text-sm font-medium">
        {{ runtime.runtime === "ready" ? "Runtime ready" : "Runtime unavailable" }}
      </p>
      <dl class="mt-4 space-y-2 text-xs text-muted-foreground">
        <div class="flex items-center justify-between gap-4">
          <dt>Database</dt>
          <dd
            :class="
              runtime.database === 'ready' ? 'text-[var(--status-healthy)]' : 'text-destructive'
            "
          >
            {{ runtime.database }}
          </dd>
        </div>
        <div class="flex items-center justify-between gap-4">
          <dt>Runtime</dt>
          <dd
            :class="
              runtime.runtime === 'ready' ? 'text-[var(--status-healthy)]' : 'text-destructive'
            "
          >
            {{ runtime.runtime }}
          </dd>
        </div>
        <div class="flex items-center justify-between gap-4">
          <dt>Worker</dt>
          <dd
            :class="
              runtime.worker === 'ready' ? 'text-[var(--status-healthy)]' : 'text-destructive'
            "
          >
            {{ runtime.worker }}
          </dd>
        </div>
      </dl>
      <dl
        v-if="runtime.metrics"
        class="mt-5 grid grid-cols-2 gap-x-5 gap-y-3 border-t border-border pt-4 text-xs"
      >
        <div>
          <dt class="text-muted-foreground">Running</dt>
          <dd class="mt-1 font-mono text-foreground">
            {{ runtime.metrics.containers_running }}/{{ runtime.metrics.containers }}
          </dd>
        </div>
        <div>
          <dt class="text-muted-foreground">Images</dt>
          <dd class="mt-1 font-mono text-foreground">{{ runtime.metrics.images }}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">CPU</dt>
          <dd class="mt-1 font-mono text-foreground">{{ runtime.metrics.cpus }}</dd>
        </div>
        <div>
          <dt class="text-muted-foreground">Memory</dt>
          <dd class="mt-1 font-mono text-foreground">
            {{ formatBytes(runtime.metrics.memory_bytes) }}
          </dd>
        </div>
      </dl>
    </div>
    <div v-else class="px-5 py-5 text-xs leading-5 text-muted-foreground">
      <CircleAlert class="mb-2 size-4" :stroke-width="1.5" />Runtime readiness could not be loaded.
    </div>
  </section>
</template>
