// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  deploy: vi.fn(),
  list: vi.fn(),
  listProject: vi.fn(),
  rollback: vi.fn(),
  stop: vi.fn(),
}));

vi.mock("@/lib/api/deployments", () => ({
  apiDeployService: api.deploy,
  apiListDeployments: api.list,
  apiListProjectDeployments: api.listProject,
  apiRollbackDeployment: api.rollback,
  apiStopService: api.stop,
}));

const deployment = {
  id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
  service_id: "a71f9bf9-b5c4-4ea6-a943-a9d3b92d97d0",
  generation: 1,
  status: "queued" as const,
  failure_reason: null,
  created_at: "2026-07-31T00:00:00Z",
  started_at: null,
  finished_at: null,
};

afterEach(() => {
  api.deploy.mockReset();
  api.list.mockReset();
  api.listProject.mockReset();
  api.rollback.mockReset();
  api.stop.mockReset();
  vi.resetModules();
});

describe("useDeployment", () => {
  it("keeps loaded service history when another request fails", async () => {
    api.list
      .mockResolvedValueOnce({ success: true, data: [deployment] })
      .mockResolvedValueOnce({ success: false, error: "Service unavailable" });
    const { useDeployment } = await import("./useDeployment");
    const deployments = useDeployment();

    await deployments.load(deployment.service_id);
    await deployments.load("other-service");

    expect(deployments.data.value).toEqual([deployment]);
    expect(deployments.error.value).toBe("Service unavailable");
  });

  it("loads bounded project history in one API request", async () => {
    api.listProject.mockResolvedValueOnce({ success: true, data: [deployment] });
    const { useDeployment } = await import("./useDeployment");
    const deployments = useDeployment();

    await deployments.loadProject("project-id");

    expect(api.listProject.mock.calls).toEqual([["project-id"]]);
    expect(deployments.data.value).toEqual([deployment]);
  });

  it("submits deploy and stop without fake runtime state", async () => {
    api.deploy.mockResolvedValueOnce({ success: true, data: deployment });
    api.stop.mockResolvedValueOnce({
      success: true,
      data: { ...deployment, status: "stopping" },
    });
    const { useDeployment } = await import("./useDeployment");
    const deployments = useDeployment();

    await deployments.deploy(deployment.service_id);
    const stopped = await deployments.stop(deployment.service_id);

    expect(stopped?.status).toBe("stopping");
    expect(deployments.data.value[0]?.status).toBe("stopping");
  });
});
