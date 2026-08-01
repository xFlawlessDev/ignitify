<script setup lang="ts">
import { computed, shallowRef } from "vue";
import type { DeploymentLog } from "@/lib/types";

const props = defineProps<{
  connected: boolean;
  logs: DeploymentLog[];
  streamError: string | null;
}>();

const filter = shallowRef<"all" | DeploymentLog["stream"]>("all");
const visibleLogs = computed(() =>
  filter.value === "all" ? props.logs : props.logs.filter((log) => log.stream === filter.value),
);
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-center justify-between gap-4 border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Output</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Logs</h2>
      </div>
      <div class="flex items-center gap-2">
        <select
          v-model="filter"
          class="h-8 border border-input bg-background px-2 text-xs"
          aria-label="Log stream filter"
        >
          <option value="all">All</option>
          <option value="stdout">stdout</option>
          <option value="stderr">stderr</option>
          <option value="system">system</option>
        </select>
        <span class="text-xs text-muted-foreground">{{ connected ? "Live" : "Reconnecting" }}</span>
      </div>
    </div>
    <p
      v-if="streamError"
      class="border-b border-border px-5 py-2 text-xs text-destructive"
      role="alert"
    >
      {{ streamError }}
    </p>
    <pre
      v-if="visibleLogs.length"
      class="max-h-[420px] overflow-auto p-5 font-mono text-xs leading-5 whitespace-pre-wrap"
    ><code v-for="log in visibleLogs" :key="log.sequence" class="block" :class="log.stream === 'stderr' ? 'text-destructive' : 'text-foreground'">{{ log.line }}</code></pre>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No retained logs</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Latest 10,000 lines per deployment remain available.
      </p>
    </div>
  </section>
</template>
