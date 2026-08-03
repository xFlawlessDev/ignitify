// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getDashboard: vi.fn(),
  getRuntimeStatus: vi.fn(),
}));

vi.mock("@/lib/api/dashboard", () => ({
  apiGetDashboard: api.getDashboard,
  apiGetRuntimeStatus: api.getRuntimeStatus,
}));

const dashboard = {
  deployments: [
    {
      created_at: "2026-08-01T00:05:00Z",
      failure_reason: null,
      finished_at: null,
      generation: 2,
      id: "active-deployment",
      service_id: "service-id",
      started_at: "2026-08-01T00:05:01Z",
      status: "running" as const,
    },
    {
      created_at: "2026-08-01T00:00:00Z",
      failure_reason: "Older failure",
      finished_at: "2026-08-01T00:01:00Z",
      generation: 1,
      id: "failed-deployment",
      service_id: "service-id",
      started_at: "2026-08-01T00:00:01Z",
      status: "failed" as const,
    },
  ],
  projects: [{ id: "project-id", name: "Web" }],
  services: [
    {
      desired_generation: 2,
      desired_state: "running" as const,
      id: "service-id",
      kind: "image" as const,
      name: "frontend",
      project_id: "project-id",
    },
  ],
};

afterEach(() => {
  api.getDashboard.mockReset();
  api.getRuntimeStatus.mockReset();
  vi.resetModules();
});

describe("useOperationsDashboard", () => {
  it("uses each service's latest deployment for workspace metrics", async () => {
    api.getDashboard.mockResolvedValueOnce({ data: dashboard, success: true });
    api.getRuntimeStatus.mockResolvedValueOnce({
      data: { database: "ready", runtime: "ready", worker: "ready" },
      success: true,
    });
    const { useOperationsDashboard } = await import("./useOperationsDashboard");
    const operations = useOperationsDashboard();

    await operations.load();

    expect(operations.metrics.value).toMatchObject({
      active: 1,
      failed: 0,
      projects: 1,
      services: 1,
    });
    expect(operations.recentDeployments.value.map((item) => item.deployment.id)).toEqual([
      "active-deployment",
      "failed-deployment",
    ]);
    expect(operations.runtime.value).toEqual({
      database: "ready",
      runtime: "ready",
      worker: "ready",
    });
  });
});
