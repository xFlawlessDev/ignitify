import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { apiGetSystemMetrics } from "@/lib/api/dashboard";
import type { SystemMetrics } from "@/lib/types";

export type MonitoringRange = "1h" | "6h" | "24h";
export type TelemetryKey =
  | "cpu"
  | "memory"
  | "disk"
  | "dockerDisk"
  | "blockRead"
  | "blockWrite"
  | "networkIn"
  | "networkOut";

export interface MonitoringSample {
  time: string;
  cpu: number;
  cpuCores: number;
  memory: number;
  memoryUsedBytes: number;
  memoryTotalBytes: number;
  disk: number;
  diskUsedBytes: number;
  diskTotalBytes: number;
  dockerDisk: number;
  dockerDiskUsedBytes: number | null;
  dockerDiskTotalBytes: number | null;
  blockRead: number;
  blockWrite: number;
  networkIn: number;
  networkOut: number;
}

export interface MonitoringMetric {
  id: TelemetryKey;
  label: string;
  value: string;
  detail: string;
  delta: string;
  deltaTone: "up" | "down" | "neutral";
  progress?: number;
  history: number[];
}

const rangeSampleLimits: Record<MonitoringRange, number> = {
  "1h": 120,
  "6h": 720,
  "24h": 2880,
};

function percentage(used: number, total: number): number {
  return total > 0 ? Math.min(Math.max((used / total) * 100, 0), 100) : 0;
}

function megabytesPerSecond(bytesPerSecond: number): number {
  return bytesPerSecond / 1024 ** 2;
}

function formatBytes(value: number): string {
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GiB`;
  if (value >= 1024 ** 2) return `${(value / 1024 ** 2).toFixed(1)} MiB`;
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${Math.round(value)} B`;
}

function formatRate(value: number): string {
  return `${value.toFixed(1)} MB/s`;
}

function percentageDelta(current: number, previous: number): string {
  if (previous === 0) return current === 0 ? "0.0%" : "+new";
  const delta = ((current - previous) / Math.abs(previous)) * 100;
  return `${delta >= 0 ? "+" : ""}${delta.toFixed(1)}%`;
}

function deltaTone(current: number, previous: number): "up" | "down" | "neutral" {
  if (current === previous) return "neutral";
  return current > previous ? "up" : "down";
}

function toSample(metrics: SystemMetrics, sampledAt: Date): MonitoringSample {
  return {
    time: new Intl.DateTimeFormat("en-GB", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hourCycle: "h23",
    }).format(sampledAt),
    cpu: Math.min(Math.max(metrics.cpu_usage_percentage, 0), 100),
    cpuCores: metrics.cpu_cores,
    memory: percentage(metrics.memory_used_bytes, metrics.memory_total_bytes),
    memoryUsedBytes: metrics.memory_used_bytes,
    memoryTotalBytes: metrics.memory_total_bytes,
    disk: percentage(metrics.disk_used_bytes, metrics.disk_total_bytes),
    diskUsedBytes: metrics.disk_used_bytes,
    diskTotalBytes: metrics.disk_total_bytes,
    dockerDisk: percentage(
      metrics.docker_disk_used_bytes ?? 0,
      metrics.docker_disk_total_bytes ?? 0,
    ),
    dockerDiskUsedBytes: metrics.docker_disk_used_bytes,
    dockerDiskTotalBytes: metrics.docker_disk_total_bytes,
    blockRead: megabytesPerSecond(metrics.block_read_bytes_per_second),
    blockWrite: megabytesPerSecond(metrics.block_write_bytes_per_second),
    networkIn: megabytesPerSecond(metrics.network_receive_bytes_per_second),
    networkOut: megabytesPerSecond(metrics.network_transmit_bytes_per_second),
  };
}

export function useSystemMonitoring() {
  const range = shallowRef<MonitoringRange>("6h");
  const samples = shallowRef<MonitoringSample[]>([]);
  const lastUpdated = shallowRef<Date | null>(null);
  const isRefreshing = shallowRef(false);
  const loading = shallowRef(true);
  const error = shallowRef<string | null>(null);
  const autoRefresh = shallowRef(true);
  let refreshTimer: number | undefined;

  const latest = computed<MonitoringSample | null>(() => samples.value.at(-1) ?? null);
  const previous = computed<MonitoringSample | null>(() => samples.value.at(-2) ?? null);

  const metrics = computed<MonitoringMetric[]>(() => {
    const current = latest.value;
    if (!current) return [];
    const prior = previous.value ?? current;
    const dockerAvailable = current.dockerDiskTotalBytes !== null;

    return [
      {
        id: "cpu",
        label: "CPU Usage",
        value: `${current.cpu.toFixed(1)}%`,
        detail: `${current.cpuCores} vCPU · latest host sample`,
        delta: percentageDelta(current.cpu, prior.cpu),
        deltaTone: deltaTone(current.cpu, prior.cpu),
        progress: current.cpu,
        history: samples.value.map((sample) => sample.cpu),
      },
      {
        id: "memory",
        label: "Memory Usage",
        value: `${formatBytes(current.memoryUsedBytes)} / ${formatBytes(current.memoryTotalBytes)}`,
        detail: `${current.memory.toFixed(1)}% used · ${formatBytes(
          Math.max(current.memoryTotalBytes - current.memoryUsedBytes, 0),
        )} available`,
        delta: percentageDelta(current.memory, prior.memory),
        deltaTone: deltaTone(current.memory, prior.memory),
        progress: current.memory,
        history: samples.value.map((sample) => sample.memory),
      },
      {
        id: "disk",
        label: "Disk Space",
        value: `${formatBytes(current.diskUsedBytes)} / ${formatBytes(current.diskTotalBytes)}`,
        detail: `${current.disk.toFixed(1)}% used · ${formatBytes(
          Math.max(current.diskTotalBytes - current.diskUsedBytes, 0),
        )} available`,
        delta: percentageDelta(current.disk, prior.disk),
        deltaTone: deltaTone(current.disk, prior.disk),
        progress: current.disk,
        history: samples.value.map((sample) => sample.disk),
      },
      {
        id: "dockerDisk",
        label: "Docker Disk Usage",
        value: dockerAvailable
          ? `${formatBytes(current.dockerDiskUsedBytes ?? 0)} / ${formatBytes(
              current.dockerDiskTotalBytes ?? 0,
            )}`
          : "Unavailable",
        detail: dockerAvailable
          ? `${current.dockerDisk.toFixed(1)}% used · Docker Engine data root`
          : "Docker Engine unavailable",
        delta: dockerAvailable ? percentageDelta(current.dockerDisk, prior.dockerDisk) : "—",
        deltaTone: dockerAvailable ? deltaTone(current.dockerDisk, prior.dockerDisk) : "neutral",
        progress: dockerAvailable ? current.dockerDisk : undefined,
        history: samples.value.map((sample) => sample.dockerDisk),
      },
      {
        id: "blockRead",
        label: "Block I/O",
        value: formatRate(current.blockRead),
        detail: `Read ${formatRate(current.blockRead)} · Write ${formatRate(current.blockWrite)}`,
        delta: percentageDelta(current.blockRead, prior.blockRead),
        deltaTone: deltaTone(current.blockRead, prior.blockRead),
        history: samples.value.map((sample) => sample.blockRead),
      },
      {
        id: "networkIn",
        label: "Network I/O",
        value: formatRate(current.networkIn),
        detail: `In ${formatRate(current.networkIn)} · Out ${formatRate(current.networkOut)}`,
        delta: percentageDelta(current.networkIn, prior.networkIn),
        deltaTone: deltaTone(current.networkIn, prior.networkIn),
        history: samples.value.map((sample) => sample.networkIn),
      },
    ];
  });

  function stopAutoRefresh() {
    if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
    refreshTimer = undefined;
  }

  function startAutoRefresh() {
    stopAutoRefresh();
    if (autoRefresh.value) refreshTimer = window.setInterval(() => void refresh(), 30_000);
  }

  async function refresh() {
    if (isRefreshing.value) return;
    isRefreshing.value = true;
    try {
      const result = await apiGetSystemMetrics();
      if (!result.success) {
        error.value = result.error ?? "Unable to load system metrics";
        return;
      }
      const sample = toSample(result.data, new Date());
      samples.value = [...samples.value, sample].slice(-rangeSampleLimits[range.value]);
      lastUpdated.value = new Date();
      error.value = null;
    } finally {
      loading.value = false;
      isRefreshing.value = false;
    }
  }

  function setRange(nextRange: MonitoringRange) {
    range.value = nextRange;
    samples.value = samples.value.slice(-rangeSampleLimits[nextRange]);
  }

  watch(autoRefresh, startAutoRefresh);

  onMounted(() => {
    void refresh();
    startAutoRefresh();
  });
  onUnmounted(stopAutoRefresh);

  return {
    autoRefresh,
    error,
    isRefreshing,
    lastUpdated,
    latest,
    loading,
    metrics,
    range,
    samples,
    refresh,
    setRange,
  };
}
