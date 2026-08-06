<script setup lang="ts">
import { CircleAlert, Container, Cpu, Layers3, RefreshCw, Server } from "@lucide/vue";
import { computed, onMounted, onUnmounted } from "vue";
import RuntimeStatusPanel from "@/components/runtime/RuntimeStatusPanel.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useRuntimeContainers } from "@/composables/useRuntimeContainers";
import { useRuntimeStatus } from "@/composables/useRuntimeStatus";
import type { RuntimePort } from "@/lib/types";

const {
  data: runtime,
  error: runtimeError,
  load: loadRuntime,
  loading: runtimeLoading,
} = useRuntimeStatus();
const {
  data: containers,
  error: inventoryError,
  load: loadContainers,
  loading: inventoryLoading,
} = useRuntimeContainers();
const metrics = computed(() => runtime.value?.metrics ?? null);
const error = computed(() => runtimeError.value ?? inventoryError.value);
const loading = computed(() => runtimeLoading.value || inventoryLoading.value);

const REFRESH_INTERVAL_MS = 3_000;
let refreshTimer: number | undefined;

function formatBytes(value: number | null | undefined) {
  if (!value) return "Unavailable";
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GiB`;
  return `${(value / 1024 ** 2).toFixed(0)} MiB`;
}

function formatCpuLimit(value: number | null) {
  if (!value) return "No limit";
  return `${(value / 1_000_000_000).toFixed(1)} CPU`;
}

function statusClass(status: string) {
  if (status.startsWith("Up")) return "text-[var(--status-healthy)]";
  if (status.startsWith("Restarting")) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function formatPort(port: RuntimePort) {
  const container = `${port.container_port}/${port.protocol}`;
  if (port.host_port === null) return container;
  return `${port.host_ip ?? "0.0.0.0"}:${port.host_port} → ${container}`;
}

function scheduleRefresh(delay = REFRESH_INTERVAL_MS) {
  if (document.visibilityState === "hidden") return;
  refreshTimer = window.setTimeout(() => {
    void refresh();
  }, delay);
}

function handleVisibilityChange() {
  if (document.visibilityState === "hidden") {
    if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
    refreshTimer = undefined;
    return;
  }
  if (refreshTimer === undefined) void refresh();
}

async function refresh() {
  if (loading.value) return;
  if (refreshTimer !== undefined) {
    window.clearTimeout(refreshTimer);
    refreshTimer = undefined;
  }
  const startedAt = Date.now();
  await Promise.all([loadRuntime(), loadContainers()]);
  scheduleRefresh(Math.max(0, REFRESH_INTERVAL_MS - (Date.now() - startedAt)));
}

onMounted(() => {
  document.addEventListener("visibilitychange", handleVisibilityChange);
  void refresh();
});

onUnmounted(() => {
  if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
  document.removeEventListener("visibilitychange", handleVisibilityChange);
});
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
    >
      <div>
        <p class="ui-label">Docker</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Containers</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Runtime health and aggregate container capacity for this host.
        </p>
      </div>
      <Button
        class="w-full shrink-0 sm:w-auto"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="refresh"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Refresh
      </Button>
    </header>

    <section
      class="mt-[22px] grid overflow-hidden divide-y divide-border border border-border bg-card sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
      aria-label="Docker capacity"
    >
      <template v-if="loading && !runtime">
        <div v-for="index in 4" :key="index" class="min-h-36 bg-background p-5">
          <Skeleton class="h-3 w-20" />
          <Skeleton class="mt-12 h-9 w-16" />
        </div>
      </template>
      <template v-else>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Containers</p>
            <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? metrics.containers : "—" }}
          </p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Running</p>
            <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? metrics.containers_running : "—" }}
          </p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Images</p>
            <Layers3 class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">{{ metrics ? metrics.images : "—" }}</p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Memory</p>
            <Cpu class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? formatBytes(metrics.memory_bytes) : "—" }}
          </p>
        </section>
      </template>
    </section>

    <section
      v-if="error"
      class="mt-4 flex items-start justify-between gap-4 border border-destructive/40 bg-card px-5 py-4 max-[640px]:flex-col"
      role="alert"
    >
      <div class="flex items-start gap-2 text-sm text-destructive">
        <CircleAlert class="mt-0.5 size-4 shrink-0" :stroke-width="1.5" />
        <p>{{ error }}</p>
      </div>
      <Button
        class="shrink-0 max-[640px]:w-full"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="refresh"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Retry
      </Button>
    </section>

    <section class="mt-[22px] grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_17rem]">
      <section class="min-w-0 border border-border bg-card">
        <div class="border-b border-border px-5 py-4">
          <p class="ui-label">Container inventory</p>
          <h2 class="mt-2 text-base font-medium">Per-container monitoring</h2>
        </div>
        <p
          v-if="inventoryLoading && containers === null"
          class="px-5 py-8 text-sm text-muted-foreground"
          role="status"
        >
          Loading container inventory...
        </p>
        <div v-else-if="containers === null" class="px-5 py-8">
          <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <p class="mt-4 text-sm font-medium">Docker inventory unavailable</p>
          <p class="mt-1 max-w-xl text-xs leading-5 text-muted-foreground">
            Docker did not return container details for this host. Check runtime connectivity, then
            refresh.
          </p>
        </div>
        <Table v-else-if="containers.length">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="min-w-48 px-5 text-xs text-muted-foreground">Container</TableHead>
              <TableHead class="min-w-56 text-xs text-muted-foreground">Image</TableHead>
              <TableHead class="min-w-40 text-xs text-muted-foreground">Status</TableHead>
              <TableHead class="min-w-24 text-xs text-muted-foreground">State</TableHead>
              <TableHead class="min-w-48 text-xs text-muted-foreground">Ports</TableHead>
              <TableHead class="min-w-24 text-xs text-muted-foreground">CPU</TableHead>
              <TableHead class="min-w-36 text-xs text-muted-foreground">Memory</TableHead>
              <TableHead class="px-5 text-right text-xs text-muted-foreground">Restarts</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="container in containers" :key="container.id">
              <TableCell class="px-5 py-3">
                <p class="max-w-56 truncate text-sm font-medium">{{ container.name }}</p>
                <p class="mt-1 font-mono text-[10px] text-muted-foreground">
                  {{ container.id.slice(0, 12) }}
                </p>
              </TableCell>
              <TableCell class="py-3">
                <p class="max-w-56 truncate font-mono text-[11px]" :title="container.image">
                  {{ container.image || "Unavailable" }}
                </p>
                <p v-if="container.managed" class="mt-1 text-[10px] text-muted-foreground">
                  Managed by Ignitify
                </p>
              </TableCell>
              <TableCell class="py-3">
                <span
                  class="flex items-center gap-2 text-xs"
                  :class="statusClass(container.status)"
                >
                  <span
                    class="status-dot"
                    :data-status="
                      container.status.startsWith('Up')
                        ? 'healthy'
                        : container.status.startsWith('Restarting')
                          ? 'live'
                          : undefined
                    "
                    aria-hidden="true"
                  />
                  <span class="max-w-36 truncate" :title="container.status">{{
                    container.status
                  }}</span>
                </span>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs text-muted-foreground">{{
                container.state
              }}</TableCell>
              <TableCell class="py-3">
                <p
                  v-if="container.ports.length"
                  class="max-w-44 truncate font-mono text-[11px]"
                  :title="container.ports.map(formatPort).join(', ')"
                >
                  {{ container.ports.map(formatPort).join(", ") }}
                </p>
                <span v-else class="text-xs text-muted-foreground">—</span>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs">
                {{
                  container.cpu_percentage === null
                    ? "—"
                    : `${container.cpu_percentage.toFixed(1)}%`
                }}
                <p class="mt-1 text-[10px] text-muted-foreground">
                  {{ formatCpuLimit(container.cpu_limit_nano_cpus) }}
                </p>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs">
                {{ formatBytes(container.memory_usage_bytes) }}
                <p class="mt-1 text-[10px] text-muted-foreground">
                  {{ formatBytes(container.memory_limit_bytes) }} limit
                </p>
              </TableCell>
              <TableCell class="px-5 py-3 text-right font-mono text-xs">{{
                container.restart_count
              }}</TableCell>
            </TableRow>
          </TableBody>
        </Table>
        <div v-else class="px-5 py-8">
          <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <p class="mt-4 text-sm font-medium">No containers found</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            Docker is reachable but has no active or stopped containers to monitor.
          </p>
        </div>
      </section>
      <RuntimeStatusPanel :runtime="runtime" />
    </section>
  </div>
</template>
