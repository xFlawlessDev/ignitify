<script setup lang="ts">
import Ansi from "ansi-to-vue3";
import { computed, nextTick, shallowRef, watch } from "vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { DeploymentLog } from "@/lib/types";

const props = defineProps<{
  connected: boolean;
  logs: DeploymentLog[];
  streamError: string | null;
  embedded?: boolean;
}>();

const filter = shallowRef<"all" | DeploymentLog["stream"]>("all");
const follow = shallowRef(true);
const output = shallowRef<HTMLElement | null>(null);
const visibleLogs = computed(() =>
  filter.value === "all" ? props.logs : props.logs.filter((log) => log.stream === filter.value),
);

watch(
  () => visibleLogs.value.length,
  async () => {
    if (!follow.value) return;
    await nextTick();
    if (output.value) output.value.scrollTop = output.value.scrollHeight;
  },
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
        <p class="ui-label">Deployment output</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Logs</h2>
      </div>
      <div class="flex flex-wrap items-center justify-end gap-3">
        <span class="font-mono text-[11px] text-muted-foreground"
          >{{ visibleLogs.length }} lines</span
        >
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
        <label class="flex items-center gap-2 text-xs text-muted-foreground">
          Follow
          <Switch :model-value="follow" @update:model-value="follow = $event" />
        </label>
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
    <div
      v-if="visibleLogs.length"
      class="overflow-hidden rounded-[6px] border border-border bg-obsidian-canvas"
      :class="props.embedded ? '' : 'mx-5 mb-5'"
    >
      <pre
        ref="output"
        class="m-0 max-h-[420px] overflow-auto p-4 font-mono text-xs leading-5 whitespace-pre-wrap"
      ><Ansi
        v-for="log in visibleLogs"
        :key="log.sequence"
        class="block"
        :class="
          log.stream === 'stderr'
            ? 'text-destructive'
            : log.stream === 'system'
              ? 'text-pale-stone'
              : 'text-chalk'
        "
      >{{ log.line }}</Ansi></pre>
    </div>
    <div v-else class="py-8" :class="props.embedded ? 'px-0' : 'px-5'">
      <p class="text-sm font-medium">
        {{ connected ? "Waiting for deployment output" : "No retained logs" }}
      </p>
      <p class="mt-1 text-xs text-muted-foreground">
        {{
          connected
            ? "Build and runtime output will appear here."
            : "Latest 10,000 lines per deployment remain available."
        }}
      </p>
    </div>
  </section>
</template>
