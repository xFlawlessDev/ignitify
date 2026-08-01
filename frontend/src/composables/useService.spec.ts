// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  list: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@/lib/api/services", () => ({
  apiCreateService: api.create,
  apiListServices: api.list,
  apiUpdateService: api.update,
}));

const service = {
  id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
  project_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
  environment_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
  role: "owner" as const,
  name: "web",
  kind: "image" as const,
  image_reference: "nginx@sha256:deadbeef",
  internal_port: 8080,
  healthcheck: null,
  desired_generation: 1,
  desired_state: "stopped" as const,
  created_at: "2026-07-31T00:00:00Z",
  updated_at: "2026-07-31T00:00:00Z",
  variables: [],
};

afterEach(() => {
  api.create.mockReset();
  api.list.mockReset();
  api.update.mockReset();
  vi.resetModules();
});

describe("useService", () => {
  it("loads records and surfaces errors", async () => {
    api.list.mockResolvedValueOnce({ success: true, data: [service] });
    const { useService } = await import("./useService");
    const services = useService();

    await services.load(service.project_id);

    expect(services.data.value).toEqual([service]);
    api.list.mockResolvedValueOnce({ success: false, data: [], error: "offline" });
    await services.load(service.project_id);
    expect(services.error.value).toBe("offline");
  });
});
