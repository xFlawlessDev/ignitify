// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ getHistory: vi.fn() }));

vi.mock("@/lib/api/uptime-monitors", () => ({
  apiGetUptimeMonitorHistory: api.getHistory,
}));

afterEach(() => {
  vi.clearAllMocks();
  vi.resetModules();
});

describe("useUptimeMonitorHistory", () => {
  it("loads the bounded 24-hour history and clears it after a monitor changes", async () => {
    api.getHistory.mockResolvedValue({
      success: true,
      data: {
        monitor_id: "monitor-1",
        retention_days: 30,
        checks: [],
        summary: {
          window_hours: 24,
          total_checks: 4,
          successful_checks: 3,
          failed_checks: 1,
          availability_percentage: 75,
          error_budget_percentage: 0,
          budget_consumed_percentage: 2500,
          status: "exhausted",
        },
      },
    });

    const { useUptimeMonitorHistory } = await import("./useUptimeMonitorHistory");
    const history = useUptimeMonitorHistory();
    await history.loadHistory("monitor-1");

    expect(api.getHistory.mock.calls).toEqual([["monitor-1", { hours: 24, limit: 500 }]]);
    expect(history.histories.value["monitor-1"]?.summary.status).toBe("exhausted");

    history.clearHistory("monitor-1");
    expect(history.histories.value["monitor-1"]).toBeUndefined();
  });
});
