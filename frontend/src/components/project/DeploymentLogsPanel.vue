<script setup lang="ts">
import { computed, shallowRef } from "vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DeploymentLog } from "@/lib/types";

const props = defineProps<{
  connected: boolean;
  logs: DeploymentLog[];
  streamError: string | null;
  embedded?: boolean;
}>();

const filter = shallowRef<"all" | DeploymentLog["stream"]>("all");
const visibleLogs = computed(() =>
  filter.value === "all" ? props.logs : props.logs.filter((log) => log.stream === filter.value),
);
</script>

<template>
  <section
    :class="
      props.embedded
        ? 'bg-transparent'
        : 'overflow-hidden rounded-[10px] border border-border bg-card'
    "
  >
    <div
      class="flex items-center justify-between gap-4 border-b border-border py-4"
      :class="props.embedded ? 'px-0' : 'px-5'"
    >
      <div>
        <p class="ui-label">Output</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Logs</h2>
      </div>
      <div class="flex items-center gap-2">
        <Select v-model="filter">
          <SelectTrigger class="h-8 w-[100px] px-2 text-xs" aria-label="Log stream filter">
            <SelectValue placeholder="All" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All</SelectItem>
            <SelectItem value="stdout">stdout</SelectItem>
            <SelectItem value="stderr">stderr</SelectItem>
            <SelectItem value="system">system</SelectItem>
          </SelectContent>
        </Select>
        <span class="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span
            class="status-dot"
            :data-status="connected ? 'live' : 'inactive'"
            aria-hidden="true"
          />
          {{ connected ? "Live" : "Reconnecting" }}
        </span>
      </div>
    </div>
    <p
      v-if="streamError"
      class="border-b border-border py-2 text-xs text-destructive"
      :class="props.embedded ? 'px-0' : 'px-5'"
      role="alert"
    >
      {{ streamError }}
    </p>
    <pre
      v-if="visibleLogs.length"
      class="max-h-[420px] overflow-auto py-4 font-mono text-xs leading-5 whitespace-pre-wrap"
      :class="props.embedded ? 'px-0' : 'px-5'"
    ><code v-for="log in visibleLogs" :key="log.sequence" class="block" :class="log.stream === 'stderr' ? 'text-destructive' : 'text-foreground'">{{ log.line }}</code></pre>
    <div v-else class="py-8" :class="props.embedded ? 'px-0' : 'px-5'">
      <p class="text-sm font-medium">No retained logs</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Latest 10,000 lines per deployment remain available.
      </p>
    </div>
  </section>
</template>
