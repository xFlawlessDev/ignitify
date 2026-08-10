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
} from "@lucide/vue";
import { computed, shallowRef, type Component, watch } from "vue";
import { toast } from "vue-sonner";
import MonitoringMetricCard from "@/components/monitoring/MonitoringMetricCard.vue";
import MonitoringTrendChart from "@/components/monitoring/MonitoringTrendChart.vue";
import DestinationSelector from "@/components/runtime/DestinationSelector.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  apiGetRemoteServerAgent,
  type RemoteServerAgentSummary,
  type RemoteServerSummary,
} from "@/lib/api";
import {
  useSystemMonitoring,
  type MonitoringRange,
  type TelemetryKey,
} from "@/composables/useSystemMonitoring";

const selectedDestinationId = shallowRef("local");
const selectedRemoteServer = shallowRef<RemoteServerSummary | null>(null);
const remoteAgent = shallowRef<RemoteServerAgentSummary | null>(null);
const remoteLoading = shallowRef(false);
const remoteError = shallowRef<string | null>(null);
const isLocalDestination = computed(() => selectedDestinationId.value === "local");

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
} = useSystemMonitoring({ enabled: isLocalDestination });

const lastNotifiedError = shallowRef<string | null>(null);

watch(error, (message) => {
  if (!message) {
    lastNotifiedError.value = null;
    return;
  }
  if (message === lastNotifiedError.value) return;
  lastNotifiedError.value = message;
  toast.error("Monitoring unavailable", { description: message });
});

watch(remoteError, (message) => {
  if (message) toast.error("Remote monitoring unavailable", { description: message });
});

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

const remoteUpdatedLabel = computed(() => {
  const heartbeat = remoteAgent.value?.last_heartbeat_at;
  if (!heartbeat) return "—";
  const date = new Date(heartbeat);
  if (Number.isNaN(date.valueOf())) return "—";
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  }).format(date);
});

function formatRemoteBytes(value: number | null) {
  if (value === null) return "Unavailable";
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GiB`;
  if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
  return `${Math.round(value / 1024)} KiB`;
}

function remoteUsage(used: number | null, total: number | null) {
  if (used === null || total === null || total <= 0) return null;
  return Math.min(Math.max((used / total) * 100, 0), 100);
}

const remoteMetricCards = computed(() => {
  const agent = remoteAgent.value;
  if (!agent) return [];
  const memoryUsage = remoteUsage(agent.memory_used_bytes, agent.memory_total_bytes);
  const diskUsage = remoteUsage(agent.disk_used_bytes, agent.disk_total_bytes);

  return [
    {
      label: "CPU usage",
      value:
        agent.cpu_usage_percentage === null
          ? "Unavailable"
          : `${agent.cpu_usage_percentage.toFixed(1)}%`,
      detail: `${agent.cpu_cores ?? "—"} vCPU`,
      progress: agent.cpu_usage_percentage ?? undefined,
    },
    {
      label: "Memory",
      value: `${formatRemoteBytes(agent.memory_used_bytes)} / ${formatRemoteBytes(
        agent.memory_total_bytes,
      )}`,
      detail: `${memoryUsage?.toFixed(1) ?? "—"}% used`,
      progress: memoryUsage ?? undefined,
    },
    {
      label: "Disk",
      value: `${formatRemoteBytes(agent.disk_used_bytes)} / ${formatRemoteBytes(agent.disk_total_bytes)}`,
      detail: `${diskUsage?.toFixed(1) ?? "—"}% used`,
      progress: diskUsage ?? undefined,
    },
    {
      label: "Docker",
      value:
        agent.docker_containers === null
          ? "Unavailable"
          : `${agent.docker_running_containers ?? 0} / ${agent.docker_containers}`,
      detail: "running containers",
      progress: undefined,
    },
  ];
});

async function loadRemoteAgent(serverId: string) {
  remoteLoading.value = true;
  remoteError.value = null;
  const result = await apiGetRemoteServerAgent(serverId);
  if (result.success) remoteAgent.value = result.data;
  else remoteError.value = result.error ?? "Unable to load remote agent metrics";
  remoteLoading.value = false;
}

async function handleDestinationChange(server: RemoteServerSummary | null) {
  selectedRemoteServer.value = server;
  remoteAgent.value = server?.agent ?? null;
  remoteError.value = null;
  if (server) await loadRemoteAgent(server.id);
}

async function refreshMonitoring() {
  if (isLocalDestination.value) {
    await refresh();
    if (!error.value) toast.success("Monitoring data refreshed");
    return;
  }
  if (selectedRemoteServer.value) {
    await loadRemoteAgent(selectedRemoteServer.value.id);
    if (!remoteError.value) toast.success("Remote monitoring data refreshed");
  }
}

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
      <div class="flex w-full flex-wrap items-center justify-end gap-2 sm:w-auto">
        <DestinationSelector
          v-model="selectedDestinationId"
          class="min-w-52 flex-1 sm:flex-none"
          @change="handleDestinationChange"
        />
        <div
          v-if="isLocalDestination"
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
          :disabled="isRefreshing || remoteLoading"
          aria-label="Refresh monitoring data"
          title="Refresh monitoring data"
          @click="refreshMonitoring"
        >
          <RefreshCw
            class="size-4"
            :class="isRefreshing ? 'animate-spin' : ''"
            :stroke-width="1.5"
          />
        </Button>
      </div>
    </header>

    <template v-if="isLocalDestination">
      <div
        class="mt-4 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground"
        role="status"
        aria-live="polite"
      >
        <span
          class="status-dot"
          :data-status="metrics.length ? 'healthy' : undefined"
          aria-hidden="true"
        />
        <span v-if="metrics.length">Live sample</span>
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
        <article
          v-for="index in 6"
          :key="index"
          class="min-w-0 app-surface px-[18px] pt-[18px] pb-4"
        >
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
    </template>

    <template v-else>
      <div
        class="mt-4 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground"
        role="status"
        aria-live="polite"
      >
        <span
          class="status-dot"
          :data-status="
            remoteError ? 'live' : remoteAgent?.status === 'online' ? 'healthy' : undefined
          "
          aria-hidden="true"
        />
        <span>{{
          remoteError ? "Metrics unavailable" : (remoteAgent?.status ?? "Agent pending")
        }}</span>
        <span class="text-border" aria-hidden="true">/</span>
        <span>Updated {{ remoteUpdatedLabel }}</span>
      </div>

      <section v-if="!remoteAgent" class="mt-6 app-surface px-5 py-8">
        <p class="ui-label">Remote telemetry</p>
        <h2 class="mt-2 text-base font-medium">Monitoring agent is not reporting</h2>
        <p class="mt-2 max-w-xl text-sm text-muted-foreground">
          Install the Ignitify agent on {{ selectedRemoteServer?.name ?? "this destination" }} to
          receive host metrics here.
        </p>
      </section>

      <section
        v-else
        class="mt-6 grid grid-cols-4 gap-4 max-[900px]:grid-cols-2 max-[560px]:grid-cols-1"
        aria-label="Remote host resource metrics"
      >
        <article
          v-for="metric in remoteMetricCards"
          :key="metric.label"
          class="min-w-0 app-surface px-[18px] pt-[18px] pb-4"
        >
          <p class="ui-label">{{ metric.label }}</p>
          <p class="mt-6 text-xl font-medium">{{ metric.value }}</p>
          <p class="mt-2 text-xs text-muted-foreground">{{ metric.detail }}</p>
          <div
            v-if="metric.progress !== undefined"
            class="mt-4 h-1 overflow-hidden rounded-full bg-muted"
          >
            <div class="h-full bg-metric-green" :style="{ width: `${metric.progress}%` }" />
          </div>
        </article>
      </section>

      <p class="mt-4 text-[11px] leading-[1.5] text-muted-foreground">
        Remote telemetry is provided by the Ignitify monitoring agent. Detailed I/O history is
        available on the destination after the agent reports those capabilities.
      </p>
    </template>
  </div>
</template>
