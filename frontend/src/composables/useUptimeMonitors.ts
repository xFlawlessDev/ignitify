import { shallowRef, triggerRef } from "vue";
import {
  apiCreateUptimeMonitor,
  apiDeleteUptimeMonitor,
  apiListUptimeMonitors,
  apiUpdateUptimeMonitor,
  type UptimeMonitorInput as ApiUptimeMonitorInput,
  type UptimeMonitorSummary,
} from "@/lib/api/uptime-monitors";

export type UptimeMonitorKind = "http" | "tcp";
export type UptimeMonitorStatus = "pending" | "up" | "down";
export type UptimeCheckState = UptimeMonitorStatus | "unknown";

export interface UptimeMonitor {
  id: string;
  name: string;
  target: string;
  kind: UptimeMonitorKind;
  intervalSeconds: number;
  enabled: boolean;
  status: UptimeMonitorStatus;
  history: UptimeCheckState[];
  latencyMs: number | null;
  lastCheckedAt: string | null;
  lastError: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface UptimeMonitorInput {
  name: string;
  target: string;
  kind: UptimeMonitorKind;
  intervalSeconds: number;
  enabled: boolean;
}

function fromApi(record: UptimeMonitorSummary): UptimeMonitor {
  return {
    id: record.id,
    name: record.name,
    target: record.target,
    kind: record.kind,
    intervalSeconds: record.interval_seconds,
    enabled: record.enabled,
    status: record.status,
    history: record.history,
    latencyMs: record.latency_ms,
    lastCheckedAt: record.last_checked_at,
    lastError: record.last_error,
    createdAt: record.created_at,
    updatedAt: record.updated_at,
  };
}

export function normalizeMonitorTarget(kind: UptimeMonitorKind, value: string): string | null {
  const target = value.trim();
  if (!target) return null;

  if (kind === "tcp") {
    const match = /^(?<hostname>[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?):(?<port>\d{1,5})$/i.exec(target);
    if (!match?.groups) return null;
    const port = Number(match.groups.port);
    return port >= 1 && port <= 65535 ? `${match.groups.hostname.toLowerCase()}:${port}` : null;
  }

  try {
    const url = new URL(target.includes("://") ? target : `https://${target}`);
    if (
      !["http:", "https:"].includes(url.protocol) ||
      !url.hostname ||
      url.username ||
      url.password ||
      url.hash
    ) {
      return null;
    }
    return url.toString();
  } catch {
    return null;
  }
}

function normalizedInput(input: UptimeMonitorInput): UptimeMonitorInput | null {
  const name = input.name.trim();
  const target = normalizeMonitorTarget(input.kind, input.target);
  const intervalSeconds = Number(input.intervalSeconds);
  if (!name || !target || !Number.isInteger(intervalSeconds) || intervalSeconds < 30) return null;
  return { ...input, name, target, intervalSeconds };
}

function toApiInput(input: UptimeMonitorInput): ApiUptimeMonitorInput {
  return {
    name: input.name,
    target: input.target,
    kind: input.kind,
    interval_seconds: input.intervalSeconds,
    enabled: input.enabled,
  };
}

const monitors = shallowRef<UptimeMonitor[]>([]);
const loading = shallowRef(false);
const refreshing = shallowRef(false);
const saving = shallowRef(false);
const error = shallowRef<string | null>(null);
const hasLoaded = shallowRef(false);

function synchronizeMonitors(records: UptimeMonitorSummary[]) {
  const existingById = new Map(monitors.value.map((monitor) => [monitor.id, monitor]));
  const nextMonitors = records.map((record) => {
    const nextMonitor = fromApi(record);
    const existingMonitor = existingById.get(nextMonitor.id);
    if (existingMonitor) {
      Object.assign(existingMonitor, nextMonitor);
      return existingMonitor;
    }
    return nextMonitor;
  });

  monitors.value.splice(0, monitors.value.length, ...nextMonitors);
  triggerRef(monitors);
}

function synchronizeMonitor(monitor: UptimeMonitor) {
  const existingMonitor = monitors.value.find((item) => item.id === monitor.id);
  if (existingMonitor) {
    Object.assign(existingMonitor, monitor);
  } else {
    monitors.value.unshift(monitor);
  }
  triggerRef(monitors);
}

export function useUptimeMonitors() {
  async function reloadMonitors() {
    const initialLoad = !hasLoaded.value;
    if (initialLoad) {
      loading.value = true;
    } else {
      refreshing.value = true;
    }
    error.value = null;
    try {
      const result = await apiListUptimeMonitors();
      if (result.success) {
        synchronizeMonitors(result.data);
      } else {
        error.value = result.error ?? "Unable to load uptime monitors.";
      }
      return result.success;
    } finally {
      hasLoaded.value = true;
      if (initialLoad) {
        loading.value = false;
      } else {
        refreshing.value = false;
      }
    }
  }

  async function addMonitor(input: UptimeMonitorInput): Promise<UptimeMonitor | null> {
    const value = normalizedInput(input);
    if (!value) return null;
    saving.value = true;
    error.value = null;
    const result = await apiCreateUptimeMonitor(toApiInput(value));
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Unable to add uptime monitor.";
      return null;
    }
    const monitor = fromApi(result.data);
    synchronizeMonitor(monitor);
    return monitor;
  }

  async function updateMonitor(
    id: string,
    input: UptimeMonitorInput,
  ): Promise<UptimeMonitor | null> {
    const value = normalizedInput(input);
    if (!value) return null;
    saving.value = true;
    error.value = null;
    const result = await apiUpdateUptimeMonitor(id, toApiInput(value));
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Unable to update uptime monitor.";
      return null;
    }
    const monitor = fromApi(result.data);
    synchronizeMonitor(monitor);
    return monitor;
  }

  async function removeMonitor(id: string): Promise<boolean> {
    saving.value = true;
    error.value = null;
    const result = await apiDeleteUptimeMonitor(id);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Unable to remove uptime monitor.";
      return false;
    }
    const index = monitors.value.findIndex((monitor) => monitor.id === id);
    if (index >= 0) {
      monitors.value.splice(index, 1);
      triggerRef(monitors);
    }
    return true;
  }

  return {
    monitors,
    loading,
    refreshing,
    saving,
    error,
    addMonitor,
    updateMonitor,
    removeMonitor,
    reloadMonitors,
  };
}
