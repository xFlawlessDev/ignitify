import { beforeEach, describe, expect, it, vi } from "vitest";
import { useServiceRuntimeLogs } from "./useServiceRuntimeLogs";

const mocks = vi.hoisted(() => ({
  getContainers: vi.fn(),
  getLogs: vi.fn(),
}));

vi.mock("@/lib/api/dashboard", () => ({
  apiGetRuntimeContainers: mocks.getContainers,
}));

vi.mock("@/lib/api/runtime-containers", () => ({
  apiGetRuntimeContainerLogs: mocks.getLogs,
}));

const service = {
  id: "service-1",
  project_id: "project-1",
  environment_id: "environment-1",
  role: "owner" as const,
  name: "web",
  kind: "image" as const,
  internal_port: 8080,
  healthcheck: null,
  desired_generation: 1,
  desired_state: "running" as const,
  created_at: "2026-08-10T00:00:00Z",
  updated_at: "2026-08-10T00:00:00Z",
  variables: [],
};

const deployment = {
  id: "deployment-1",
  correlation_id: "correlation-1",
  service_id: service.id,
  generation: 1,
  status: "healthy" as const,
  failure_reason: null,
  attempt_count: 1,
  retry_after: null,
  cancel_requested_at: null,
  supply_chain_report: null,
  approval: { status: "not_required" as const },
  created_at: "2026-08-10T00:00:00Z",
  started_at: "2026-08-10T00:00:00Z",
  finished_at: null,
};

function container(name: string, managed = true) {
  return {
    id: `container-${name}`,
    name,
    image: "nginx:latest",
    state: "running",
    status: "Up 1 minute",
    health: null,
    ports: [],
    restart_count: 0,
    cpu_percentage: null,
    memory_usage_bytes: null,
    cpu_limit_nano_cpus: null,
    memory_limit_bytes: null,
    managed,
  };
}

describe("useServiceRuntimeLogs", () => {
  beforeEach(() => {
    mocks.getContainers.mockReset();
    mocks.getLogs.mockReset();
  });

  it("loads logs from the managed container for the active image deployment", async () => {
    const expected = container("ignitify-svc-service-1-g1");
    mocks.getContainers.mockResolvedValueOnce({
      success: true,
      data: { containers: [container("unmanaged", false), expected] },
    });
    mocks.getLogs.mockResolvedValueOnce({ success: true, data: { logs: "server started" } });

    const logs = useServiceRuntimeLogs();
    await logs.load(service, deployment);

    expect(mocks.getContainers.mock.calls).toEqual([[undefined]]);
    expect(mocks.getLogs.mock.calls).toEqual([[expected.id, undefined]]);
    expect(logs.container.value).toEqual(expected);
    expect(logs.output.value).toBe("server started");
  });

  it("keeps the empty state when the active deployment has no managed container", async () => {
    mocks.getContainers.mockResolvedValueOnce({
      success: true,
      data: { containers: [container("unmanaged", false)] },
    });

    const logs = useServiceRuntimeLogs();
    await logs.load(service, deployment);

    expect(mocks.getLogs).not.toHaveBeenCalled();
    expect(logs.output.value).toBeNull();
    expect(logs.emptyState.value).toBe("no_container");
  });
});
