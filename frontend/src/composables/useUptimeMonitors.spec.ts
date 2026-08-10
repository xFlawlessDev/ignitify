// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  list: vi.fn(),
  create: vi.fn(),
  update: vi.fn(),
  remove: vi.fn(),
}));

vi.mock("@/lib/api/uptime-monitors", () => ({
  apiListUptimeMonitors: api.list,
  apiCreateUptimeMonitor: api.create,
  apiUpdateUptimeMonitor: api.update,
  apiDeleteUptimeMonitor: api.remove,
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.resetModules();
});

describe("useUptimeMonitors", () => {
  it("normalizes external HTTP and TCP monitor targets", async () => {
    const { normalizeMonitorTarget } = await import("./useUptimeMonitors");

    expect(normalizeMonitorTarget("http", "status.example.com/health")).toBe(
      "https://status.example.com/health",
    );
    expect(normalizeMonitorTarget("tcp", "Cache.EXAMPLE.com:6379")).toBe("cache.example.com:6379");
    expect(normalizeMonitorTarget("http", "ftp://status.example.com")).toBeNull();
    expect(normalizeMonitorTarget("tcp", "cache.example.com:70000")).toBeNull();
  });

  it("creates, updates, reloads, and removes API-backed monitor configuration", async () => {
    const summary = {
      id: "monitor-1",
      name: "Customer portal",
      target: "https://portal.example.com/health",
      kind: "http" as const,
      interval_seconds: 60,
      enabled: true,
      status: "pending" as const,
      history: Array.from({ length: 30 }, () => "unknown" as const),
      latency_ms: null,
      last_checked_at: null,
      last_error: null,
      created_at: "2026-08-10T00:00:00Z",
      updated_at: "2026-08-10T00:00:00Z",
    };
    api.create.mockResolvedValue({ success: true, data: summary });
    api.update.mockResolvedValue({
      success: true,
      data: { ...summary, name: "Customer portal API", enabled: false, interval_seconds: 300 },
    });
    api.list.mockResolvedValue({ success: true, data: [summary] });
    api.remove.mockResolvedValue({ success: true, data: null });

    const { useUptimeMonitors } = await import("./useUptimeMonitors");
    const uptime = useUptimeMonitors();
    const created = await uptime.addMonitor({
      name: "Customer portal",
      target: "https://portal.example.com/health",
      kind: "http",
      intervalSeconds: 60,
      enabled: true,
    });

    expect(created?.status).toBe("pending");
    expect(api.create.mock.calls[0]?.[0]).toEqual({
      name: "Customer portal",
      target: "https://portal.example.com/health",
      kind: "http",
      interval_seconds: 60,
      enabled: true,
    });

    const updated = await uptime.updateMonitor(created?.id ?? "", {
      name: "Customer portal API",
      target: "portal.example.com/health",
      kind: "http",
      intervalSeconds: 300,
      enabled: false,
    });
    expect(updated?.name).toBe("Customer portal API");
    expect(updated?.enabled).toBeFalsy();

    await uptime.reloadMonitors();
    expect(uptime.monitors.value).toHaveLength(1);
    expect(await uptime.removeMonitor("monitor-1")).toBeTruthy();
    expect(uptime.monitors.value).toHaveLength(0);
  });

  it("keeps existing monitor cards mounted during a background refresh", async () => {
    const summary = {
      id: "monitor-1",
      name: "Customer portal",
      target: "https://portal.example.com/health",
      kind: "http" as const,
      interval_seconds: 60,
      enabled: true,
      status: "up" as const,
      history: Array.from({ length: 30 }, () => "up" as const),
      latency_ms: 42,
      last_checked_at: "2026-08-10T00:00:00Z",
      last_error: null,
      created_at: "2026-08-10T00:00:00Z",
      updated_at: "2026-08-10T00:00:00Z",
    };
    api.list.mockResolvedValueOnce({ success: true, data: [summary] });

    const { useUptimeMonitors } = await import("./useUptimeMonitors");
    const uptime = useUptimeMonitors();
    await uptime.reloadMonitors();
    const existingMonitor = uptime.monitors.value[0];

    let resolveRefresh: ((value: unknown) => void) | undefined;
    api.list.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveRefresh = resolve;
        }),
    );
    const refresh = uptime.reloadMonitors();

    expect(uptime.loading.value).toBeFalsy();
    expect(uptime.refreshing.value).toBeTruthy();
    resolveRefresh?.({
      success: true,
      data: [{ ...summary, interval_seconds: 300, updated_at: "2026-08-10T00:05:00Z" }],
    });
    await refresh;

    expect(uptime.monitors.value[0]).toBe(existingMonitor);
    expect(uptime.monitors.value[0]?.intervalSeconds).toBe(300);
    expect(uptime.refreshing.value).toBeFalsy();
  });

  it("does not replace a monitor created while a reload is in flight", async () => {
    const summary = {
      id: "monitor-1",
      name: "Customer portal",
      target: "https://portal.example.com/health",
      kind: "http" as const,
      interval_seconds: 60,
      enabled: true,
      status: "pending" as const,
      history: Array.from({ length: 30 }, () => "unknown" as const),
      latency_ms: null,
      last_checked_at: null,
      last_error: null,
      created_at: "2026-08-10T00:00:00Z",
      updated_at: "2026-08-10T00:00:00Z",
    };
    let resolveList: ((value: unknown) => void) | undefined;
    api.list.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveList = resolve;
        }),
    );
    api.create.mockResolvedValue({ success: true, data: summary });

    const { useUptimeMonitors } = await import("./useUptimeMonitors");
    const uptime = useUptimeMonitors();
    const reload = uptime.reloadMonitors();
    await uptime.addMonitor({
      name: summary.name,
      target: summary.target,
      kind: summary.kind,
      intervalSeconds: summary.interval_seconds,
      enabled: summary.enabled,
    });

    resolveList?.({ success: true, data: [] });
    await reload;

    expect(uptime.monitors.value).toHaveLength(1);
    expect(uptime.monitors.value[0]?.id).toBe(summary.id);
    expect(uptime.monitors.value[0]?.name).toBe(summary.name);
  });
});
