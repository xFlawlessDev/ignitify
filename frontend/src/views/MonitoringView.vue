<script setup lang="ts">
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Container,
  Cpu,
  HardDrive,
  MemoryStick,
  Network,
  RefreshCw,
  TriangleAlert,
} from "@lucide/vue";
import { computed, type Component } from "vue";
import MonitoringMetricCard from "@/components/monitoring/MonitoringMetricCard.vue";
import MonitoringTrendChart from "@/components/monitoring/MonitoringTrendChart.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useSystemMonitoring,
  type MonitoringRange,
  type TelemetryKey,
} from "@/composables/useSystemMonitoring";

const {
  autoRefresh,
  error,
  isRefreshing,
  lastUpdated,
  loading,
  metrics,
  range,
  samples,
  refresh,
  setRange,
} = useSystemMonitoring();

const metricIcons: Record<TelemetryKey, Component> = {
  cpu: Cpu,
  memory: MemoryStick,
  disk: HardDrive,
  dockerDisk: Container,
  blockRead: Activity,
  blockWrite: Activity,
  networkIn: Network,
  networkOut: Network,
} as const;

const rangeOptions: { value: MonitoringRange; label: string }[] = [
  { value: "1h", label: "1 hour" },
  { value: "6h", label: "6 hours" },
  { value: "24h", label: "24 hours" },
];

const updatedLabel = computed(() => {
  if (!lastUpdated.value) return "—";
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  }).format(lastUpdated.value);
});

const networkMetric = computed(() => metrics.value.find((metric) => metric.id === "networkIn"));
const networkOutLabel = computed(() => {
  const sample = samples.value.at(-1);
  return sample ? `${sample.networkOut.toFixed(1)} MB/s` : "—";
});

function updateRange(nextRange: MonitoringRange) {
  setRange(nextRange);
}
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Operations</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Monitoring</h1>
        <p class="mt-2 text-sm text-muted-foreground">
          Host resource utilization and I/O throughput across the control plane.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <div
          class="flex min-w-0 overflow-hidden rounded-[4px] border border-border bg-card max-[560px]:flex-1"
          role="group"
          aria-label="Monitoring time range"
        >
          <Button
            variant="ghost"
            v-for="option in rangeOptions"
            :key="option.value"
            type="button"
            class="min-h-8 whitespace-nowrap px-[11px] font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground max-[560px]:flex-1 max-[560px]:px-[7px]"
            :class="range === option.value ? 'bg-muted text-foreground' : ''"
            :aria-pressed="range === option.value"
            @click="updateRange(option.value)"
          >
            {{ option.label }}
          </Button>
        </div>
        <Button
          class="shrink-0"
          size="icon-sm"
          variant="outline"
          :disabled="isRefreshing"
          aria-label="Refresh monitoring data"
          title="Refresh monitoring data"
          @click="refresh"
        >
          <RefreshCw
            class="size-4"
            :class="isRefreshing ? 'animate-spin' : ''"
            :stroke-width="1.5"
          />
        </Button>
      </div>
    </header>

    <div
      class="mt-4 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground"
      role="status"
      aria-live="polite"
    >
      <span
        class="status-dot"
        :data-status="error ? 'live' : metrics.length ? 'healthy' : undefined"
        aria-hidden="true"
      />
      <span v-if="error">Metrics unavailable</span>
      <span v-else-if="metrics.length">Live sample</span>
      <Skeleton v-else class="h-2.5 w-14" />
      <span class="text-border" aria-hidden="true">/</span>
      <span>Updated {{ updatedLabel }}</span>
      <Button
        variant="ghost"
        class="ml-2 inline-flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground transition-colors hover:text-foreground"
        type="button"
        @click="autoRefresh = !autoRefresh"
      >
        <span
          class="relative h-2.5 w-[18px] rounded-full border border-border bg-muted"
          aria-hidden="true"
        >
          <span
            class="absolute top-px left-px size-1.5 rounded-full bg-muted-foreground transition-[transform,background-color] duration-150 motion-reduce:transition-none"
            :class="autoRefresh ? 'translate-x-2 bg-metric-green' : ''"
          />
        </span>
        Auto-refresh {{ autoRefresh ? "on" : "off" }}
      </Button>
    </div>

    <section
      v-if="error"
      class="mt-4 flex items-center gap-2.5 rounded-[10px] border border-border px-3.5 py-3 text-[11px] text-signal-orange"
      role="alert"
    >
      <TriangleAlert class="size-4 shrink-0" :stroke-width="1.5" />
      <p>{{ error }}</p>
      <Button
        class="ml-auto shrink-0"
        size="sm"
        variant="outline"
        :disabled="isRefreshing"
        @click="refresh"
      >
        Retry
      </Button>
    </section>

    <section
      v-if="metrics.length"
      class="mt-6 grid grid-cols-3 gap-4 max-[900px]:grid-cols-2 max-[560px]:grid-cols-1"
      aria-label="System resource metrics"
    >
      <MonitoringMetricCard
        v-for="metric in metrics"
        :key="metric.id"
        :label="metric.label"
        :value="metric.value"
        :detail="metric.detail"
        :delta="metric.delta"
        :delta-tone="metric.deltaTone"
        :history="metric.history"
        :progress="metric.progress"
        :icon="metricIcons[metric.id]"
      />
    </section>

    <section
      v-else-if="loading"
      class="mt-6 grid grid-cols-3 gap-4 max-[900px]:grid-cols-2 max-[560px]:grid-cols-1"
      role="status"
      aria-label="Loading current system metrics"
    >
      <article v-for="index in 6" :key="index" class="min-w-0 app-surface px-[18px] pt-[18px] pb-4">
        <div class="flex items-center justify-between gap-3">
          <Skeleton class="h-2.5 w-24" />
          <Skeleton class="h-2.5 w-10" />
        </div>
        <Skeleton class="mt-6 h-6 w-32" />
        <Skeleton class="mt-2 h-2.5 w-full" />
        <Skeleton class="mt-4 h-1 w-full" />
      </article>
    </section>

    <section
      class="mt-4 grid grid-cols-[minmax(0,1.15fr)_minmax(0,0.85fr)] gap-4 max-[900px]:grid-cols-1"
    >
      <article class="min-w-0 app-surface p-[18px]">
        <div
          class="app-panel-header -mx-[18px] -mt-[18px] mb-[18px] flex min-h-12 items-start justify-between gap-4 px-[18px] py-[18px]"
        >
          <div>
            <p class="ui-label">Resource pressure</p>
            <h2 class="mt-2 text-base font-medium">Host utilization</h2>
          </div>
          <span class="font-mono text-[10px] text-muted-foreground">{{ range }} window</span>
        </div>
        <template v-if="samples.length">
          <MonitoringTrendChart
            :samples="samples"
            :max="100"
            unit=""
            :series="[
              { key: 'cpu', label: 'CPU', color: 'signal' },
              { key: 'memory', label: 'Memory', color: 'healthy' },
              { key: 'disk', label: 'Disk', color: 'neutral' },
            ]"
          />
        </template>
        <Skeleton v-else class="h-[220px] w-full" />
      </article>

      <article class="min-w-0 app-surface p-[18px]">
        <div
          class="app-panel-header -mx-[18px] -mt-[18px] mb-[18px] flex min-h-12 items-start justify-between gap-4 px-[18px] py-[18px]"
        >
          <div>
            <p class="ui-label">I/O throughput</p>
            <h2 class="mt-2 text-base font-medium">Read, write, and transfer</h2>
          </div>
          <Activity class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </div>
        <template v-if="samples.length">
          <MonitoringTrendChart
            :samples="samples"
            unit="MB/s"
            :series="[
              { key: 'blockRead', label: 'Disk read', color: 'signal' },
              { key: 'blockWrite', label: 'Disk write', color: 'secondary' },
              { key: 'networkIn', label: 'Network in', color: 'healthy' },
              { key: 'networkOut', label: 'Network out', color: 'neutral' },
            ]"
          />
        </template>
        <Skeleton v-else class="h-[220px] w-full" />
      </article>
    </section>

    <section
      v-if="metrics.length"
      class="mt-4 flex items-center justify-between gap-5 border-y border-border px-0.5 py-3.5 max-[560px]:items-start max-[560px]:flex-col"
    >
      <div class="flex min-w-0 items-center gap-2.5">
        <span class="status-dot" data-status="healthy" aria-hidden="true" />
        <p class="text-[11px] leading-[1.5] text-muted-foreground">
          <strong class="font-medium text-foreground">Current sample is healthy.</strong>
          Resource pressure is below the configured alert threshold.
        </p>
      </div>
      <div
        class="flex flex-none items-center gap-4 font-mono text-[10px] text-muted-foreground max-[560px]:flex-wrap"
      >
        <span class="flex items-center gap-1.5">
          <ArrowDown class="size-3.5" :stroke-width="1.5" />
          In {{ networkMetric?.value ?? "—" }}
        </span>
        <span class="flex items-center gap-1.5">
          <ArrowUp class="size-3.5" :stroke-width="1.5" />
          Out {{ networkOutLabel }}
        </span>
      </div>
    </section>
  </div>
</template>
