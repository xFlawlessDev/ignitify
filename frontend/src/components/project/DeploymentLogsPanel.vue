<script setup lang="ts">
import { BotMessageSquare, Search } from "@lucide/vue";
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Terminal } from "@/components/terminal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useAiChat } from "@/composables/useAiChat";
import type { DeploymentLog } from "@/lib/types";

const props = defineProps<{
  connected: boolean;
  logs: DeploymentLog[];
  streamError: string | null;
  embedded?: boolean;
}>();

const { t } = useI18n();
const { askAiAboutLogs } = useAiChat();
const filter = shallowRef<"all" | DeploymentLog["stream"]>("all");
const follow = shallowRef(true);
const search = shallowRef("");
const searchQuery = computed(() => search.value.trim().toLocaleLowerCase());
const visibleLogs = computed(() =>
  props.logs.filter(
    (log) =>
      (filter.value === "all" || log.stream === filter.value) &&
      (!searchQuery.value || log.line.toLocaleLowerCase().includes(searchQuery.value)),
  ),
);
const hasLogFilter = computed(() => filter.value !== "all" || Boolean(searchQuery.value));
const terminalOutput = computed(() => visibleLogs.value.map((log) => log.line).join("\n"));

function askAboutVisibleLogs() {
  askAiAboutLogs(t("ai.logContext.deployment"), terminalOutput.value);
}
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
        <label class="relative block w-full sm:w-52">
          <span class="sr-only">{{ t("deploymentLogs.search") }}</span>
          <Search
            class="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            v-model="search"
            type="search"
            class="h-8 w-full rounded-[3px] pr-2 pl-8 text-xs"
            :placeholder="t('deploymentLogs.search')"
          />
        </label>
        <span class="font-mono text-[11px] text-muted-foreground">{{
          t("deploymentLogs.lineCount", { shown: visibleLogs.length, total: logs.length })
        }}</span>
        <Button
          class="h-8 px-2 text-xs"
          size="sm"
          variant="outline"
          :disabled="visibleLogs.length === 0"
          @click="askAboutVisibleLogs"
        >
          <BotMessageSquare class="size-3.5" :stroke-width="1.5" />
          {{ t("ai.actions.ask") }}
        </Button>
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
    <Terminal
      v-if="visibleLogs.length"
      :auto-scroll="follow"
      :class="props.embedded ? 'rounded-[6px]' : 'mx-5 mb-5 rounded-[6px]'"
      :copy-label="t('deploymentLogs.copy')"
      :is-streaming="connected"
      :output="terminalOutput"
      :title="t('deploymentLogs.title')"
    />
    <div v-else class="py-8" :class="props.embedded ? 'px-0' : 'px-5'">
      <p class="text-sm font-medium">
        {{
          hasLogFilter
            ? t("deploymentLogs.noMatches")
            : connected
              ? "Waiting for deployment output"
              : "No retained logs"
        }}
      </p>
      <p v-if="!hasLogFilter" class="mt-1 text-xs text-muted-foreground">
        {{
          connected
            ? "Build and runtime output will appear here."
            : "Latest 10,000 lines per deployment remain available."
        }}
      </p>
    </div>
  </section>
</template>
