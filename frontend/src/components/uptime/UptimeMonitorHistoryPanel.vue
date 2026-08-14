<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { UptimeCheckHistoryEntry, UptimeMonitorHistory } from "@/lib/api/uptime-monitors";

const props = defineProps<{
  history?: UptimeMonitorHistory;
  loading: boolean;
  error: string | null;
  windowHours: number;
}>();

const emit = defineEmits<{ selectWindow: [hours: number] }>();

const { t } = useI18n();

const recentChecks = computed(() => props.history?.checks.slice(-8).reverse() ?? []);
const windows = [24, 24 * 7, 24 * 30] as const;

function percentage(value: number | null | undefined): string {
  return value === null || value === undefined ? "-" : `${value.toFixed(2)}%`;
}

function statusClass(status: string): string {
  if (status === "up" || status === "healthy") return "text-[var(--status-healthy)]";
  if (status === "down" || status === "exhausted") return "text-destructive";
  if (status === "warning") return "text-signal-orange";
  return "text-muted-foreground";
}

function checkClass(check: UptimeCheckHistoryEntry): string {
  return check.status === "up" ? "bg-[var(--status-healthy)]" : "bg-destructive";
}

function formatCheckTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.valueOf())) return "-";
  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(date);
}

function windowLabel(hours: number): string {
  if (hours < 48) return "24H";
  if (hours < 24 * 30) return "7D";
  return "30D";
}
</script>

<template>
  <section class="border-t border-border px-4 py-4 sm:px-5" aria-live="polite">
    <div class="flex flex-wrap items-start justify-between gap-2">
      <div>
        <p class="font-mono text-[10px] text-muted-foreground uppercase">
          {{ t("uptimeHistory.title") }}
        </p>
        <p v-if="history" class="mt-1 text-xs text-muted-foreground">
          {{ t("uptimeHistory.retention", { days: history.retention_days }) }}
        </p>
      </div>
      <div class="flex items-center gap-3">
        <div
          class="flex overflow-hidden rounded-[3px] border border-border"
          role="group"
          :aria-label="t('uptimeHistory.windowSelector')"
        >
          <Button
            v-for="hours in windows"
            :key="hours"
            size="sm"
            variant="ghost"
            type="button"
            class="h-7 min-w-9 rounded-none px-2 font-mono text-[10px]"
            :class="windowHours === hours ? 'bg-muted text-foreground' : 'text-muted-foreground'"
            :disabled="loading"
            :aria-pressed="windowHours === hours"
            @click="emit('selectWindow', hours)"
          >
            {{ windowLabel(hours) }}
          </Button>
        </div>
        <p
          v-if="history"
          class="font-mono text-xs uppercase"
          :class="statusClass(history.summary.status)"
        >
          {{ t(`uptimeHistory.status.${history.summary.status}`) }}
        </p>
      </div>
    </div>

    <div v-if="loading" class="mt-4 h-20 animate-pulse rounded-[3px] bg-muted" role="status" />
    <p v-else-if="error" class="mt-4 text-xs text-destructive" role="alert">{{ error }}</p>
    <template v-else-if="history">
      <div
        class="mt-4 grid divide-y divide-border border-y border-border sm:grid-cols-3 sm:divide-x sm:divide-y-0"
      >
        <div class="min-w-0 py-2 sm:pr-3">
          <p class="font-mono text-[10px] text-muted-foreground uppercase">
            {{ t("uptimeHistory.availability") }}
          </p>
          <p class="mt-1 font-mono text-sm">
            {{ percentage(history.summary.availability_percentage) }}
          </p>
        </div>
        <div class="min-w-0 py-2 sm:px-3">
          <p class="font-mono text-[10px] text-muted-foreground uppercase">
            {{ t("uptimeHistory.budgetConsumed") }}
          </p>
          <p class="mt-1 font-mono text-sm" :class="statusClass(history.summary.status)">
            {{ percentage(history.summary.budget_consumed_percentage) }}
          </p>
        </div>
        <div class="min-w-0 py-2 sm:pl-3">
          <p class="font-mono text-[10px] text-muted-foreground uppercase">
            {{ t("uptimeHistory.checks") }}
          </p>
          <p class="mt-1 font-mono text-sm">
            {{ history.summary.successful_checks }}/{{ history.summary.total_checks }}
          </p>
        </div>
      </div>

      <div v-if="history.checks.length" class="mt-4">
        <div class="flex items-center justify-between gap-3">
          <p class="font-mono text-[10px] text-muted-foreground uppercase">
            {{ t("uptimeHistory.timeline") }}
          </p>
          <p class="font-mono text-[10px] text-muted-foreground">
            {{ t("uptimeHistory.window", { hours: history.summary.window_hours }) }}
          </p>
        </div>
        <div class="mt-2 flex h-5 gap-px" :aria-label="t('uptimeHistory.timeline')">
          <span
            v-for="check in history.checks"
            :key="`${check.checked_at}-${check.status}`"
            class="min-w-px flex-1 rounded-[1px]"
            :class="checkClass(check)"
            :title="`${formatCheckTime(check.checked_at)}: ${check.status}`"
          />
        </div>
      </div>
      <p v-else class="mt-4 text-xs text-muted-foreground">{{ t("uptimeHistory.noData") }}</p>

      <ol v-if="recentChecks.length" class="mt-4 divide-y divide-border border-y border-border">
        <li
          v-for="check in recentChecks"
          :key="check.checked_at"
          class="grid gap-1 py-2 sm:grid-cols-[auto_minmax(0,1fr)_auto] sm:items-center sm:gap-3"
        >
          <span class="font-mono text-[10px] uppercase" :class="statusClass(check.status)">
            {{ check.status }}
          </span>
          <p class="truncate text-[11px] text-muted-foreground" :title="check.error ?? undefined">
            {{ check.error ?? formatCheckTime(check.checked_at) }}
          </p>
          <span class="font-mono text-[10px] text-muted-foreground">
            {{ check.latency_ms === null ? "-" : `${check.latency_ms} ms` }}
          </span>
        </li>
      </ol>
    </template>
  </section>
</template>
