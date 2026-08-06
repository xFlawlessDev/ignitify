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
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[700px]:items-start max-[700px]:flex-col"
    >
      <div>
        <p class="ui-label">Operations</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Monitoring</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Host resource utilization and I/O throughput across the control plane.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <div class="range-control" role="group" aria-label="Monitoring time range">
          <button
            v-for="option in rangeOptions"
            :key="option.value"
            type="button"
            class="range-control__button"
            :class="{ 'range-control__button--active': range === option.value }"
            :aria-pressed="range === option.value"
            @click="updateRange(option.value)"
          >
            {{ option.label }}
          </button>
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

    <div class="monitoring-status" role="status" aria-live="polite">
      <span
        class="status-dot"
        :data-status="error ? 'live' : metrics.length ? 'healthy' : undefined"
        aria-hidden="true"
      />
      <span v-if="error">Metrics unavailable</span>
      <span v-else-if="metrics.length">Live sample</span>
      <Skeleton v-else class="h-2.5 w-14" />
      <span class="monitoring-status__divider" aria-hidden="true">/</span>
      <span>Updated {{ updatedLabel }}</span>
      <button class="monitoring-status__auto" type="button" @click="autoRefresh = !autoRefresh">
        <span class="monitoring-status__switch" :data-active="autoRefresh" aria-hidden="true" />
        Auto-refresh {{ autoRefresh ? "on" : "off" }}
      </button>
    </div>

    <section v-if="error" class="monitoring-error" role="alert">
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

    <section v-if="metrics.length" class="metric-grid" aria-label="System resource metrics">
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
      class="metric-grid"
      role="status"
      aria-label="Loading current system metrics"
    >
      <article v-for="index in 6" :key="index" class="metric-card">
        <div class="flex items-center justify-between gap-3">
          <Skeleton class="h-2.5 w-24" />
          <Skeleton class="h-2.5 w-10" />
        </div>
        <Skeleton class="mt-6 h-6 w-32" />
        <Skeleton class="mt-2 h-2.5 w-full" />
        <Skeleton class="mt-4 h-1 w-full" />
      </article>
    </section>

    <section class="monitoring-grid">
      <article class="monitoring-panel monitoring-panel--wide">
        <div class="monitoring-panel__header">
          <div>
            <p class="ui-label">Resource pressure</p>
            <h2 class="mt-2 text-base font-medium">Host utilization</h2>
          </div>
          <span class="monitoring-panel__window">{{ range }} window</span>
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

      <article class="monitoring-panel">
        <div class="monitoring-panel__header">
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

    <section v-if="metrics.length" class="monitoring-footer">
      <div class="monitoring-footer__summary">
        <span class="status-dot" data-status="healthy" aria-hidden="true" />
        <p>
          <strong>Current sample is healthy.</strong>
          Resource pressure is below the configured alert threshold.
        </p>
      </div>
      <div class="monitoring-footer__io">
        <span
          ><ArrowDown class="size-3.5" :stroke-width="1.5" /> In
          {{ networkMetric?.value ?? "—" }}</span
        >
        <span><ArrowUp class="size-3.5" :stroke-width="1.5" /> Out {{ networkOutLabel }}</span>
      </div>
    </section>
  </div>
</template>

<style scoped>
.range-control {
  display: flex;
  min-width: 0;
  overflow: hidden;
  border: 1px solid var(--border);
  background: var(--card);
}

.range-control__button {
  min-height: 32px;
  padding: 0 11px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
  white-space: nowrap;
}

.range-control__button:hover,
.range-control__button--active {
  background: var(--muted);
  color: var(--foreground);
}

.monitoring-status {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 16px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.monitoring-status__divider {
  color: var(--border);
}

.monitoring-status__auto {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-left: 8px;
  color: var(--muted-foreground);
  font-size: 10px;
}

.monitoring-status__auto:hover {
  color: var(--foreground);
}

.monitoring-status__switch {
  width: 18px;
  height: 10px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--muted);
}

.monitoring-status__switch::after {
  display: block;
  width: 6px;
  height: 6px;
  margin: 1px;
  border-radius: 50%;
  background: var(--muted-foreground);
  content: "";
  transition:
    transform 150ms ease,
    background 150ms ease;
}

.monitoring-status__switch[data-active="true"]::after {
  background: var(--status-healthy);
  transform: translateX(8px);
}

.monitoring-error,
.monitoring-empty {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 18px;
  border: 1px solid var(--border);
  padding: 12px 14px;
  color: var(--muted-foreground);
  font-size: 11px;
}

.monitoring-error {
  color: var(--status-live);
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
  margin-top: 18px;
}

.monitoring-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, 0.85fr);
  gap: 12px;
  margin-top: 12px;
}

.monitoring-panel {
  min-width: 0;
  border: 1px solid var(--border);
  background: var(--card);
  padding: 18px;
}

.monitoring-panel__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  min-height: 48px;
  margin-bottom: 18px;
}

.monitoring-panel__window {
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.monitoring-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-top: 12px;
  border-top: 1px solid var(--border);
  border-bottom: 1px solid var(--border);
  padding: 14px 2px;
}

.monitoring-footer__summary,
.monitoring-footer__io,
.monitoring-footer__io span {
  display: flex;
  align-items: center;
}

.monitoring-footer__summary {
  min-width: 0;
  gap: 10px;
}

.monitoring-footer__summary p {
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
}

.monitoring-footer__summary strong {
  color: var(--foreground);
  font-weight: 500;
}

.monitoring-footer__io {
  flex: none;
  gap: 16px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.monitoring-footer__io span {
  gap: 5px;
}

@media (max-width: 900px) {
  .metric-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .monitoring-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 560px) {
  .range-control {
    flex: 1;
  }

  .range-control__button {
    flex: 1;
    padding: 0 7px;
  }

  .metric-grid {
    grid-template-columns: 1fr;
  }

  .monitoring-footer {
    align-items: flex-start;
    flex-direction: column;
  }

  .monitoring-footer__io {
    flex-wrap: wrap;
  }
}

@media (prefers-reduced-motion: reduce) {
  .monitoring-status__switch::after {
    transition: none;
  }
}
</style>
