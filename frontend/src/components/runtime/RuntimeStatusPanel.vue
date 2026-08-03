<script setup lang="ts">
import { CircleAlert, Server } from "@lucide/vue";
import type { RuntimeStatus } from "@/lib/types";

defineProps<{ runtime: RuntimeStatus | null }>();

function formatBytes(value: number) {
  if (!value) return "Unavailable";
  return `${(value / 1024 ** 3).toFixed(1)} GiB`;
}
</script>

<template>
  <section class="border border-border bg-card">
    <div class="border-b border-border px-5 py-4">
      <p class="ui-label">Runtime</p>
      <h2 class="mt-2 text-base font-medium">Host visibility</h2>
    </div>
    <div v-if="runtime" class="px-5 py-5">
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
