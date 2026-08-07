// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  get: vi.fn(),
  list: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@/lib/api/services", () => ({
  apiCreateService: api.create,
  apiGetService: api.get,
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
  image_reference: "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
  api.get.mockReset();
  api.list.mockReset();
  api.update.mockReset();
  localStorage.clear();
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

  it("retains application source metadata returned by the API", async () => {
    api.create.mockResolvedValueOnce({
      success: true,
      data: {
        ...service,
        source_config: {
          source: "application",
          provider_id: "provider-1",
          repository: "acme/site",
          branch: "main",
          builder: "railpack",
        },
      },
    });
    const { useService } = await import("./useService");
    const services = useService();

    const created = await services.create(service.project_id, {
      name: "web",
      kind: "image",
      image_reference: service.image_reference,
      internal_port: 8080,
      healthcheck: null,
      variables: [],
      source_config: {
        source: "application",
        provider_id: "provider-1",
        repository: "acme/site",
        branch: "main",
        builder: "railpack",
      },
    });

    expect(created?.source_config?.repository).toBe("acme/site");
    expect(services.data.value[0]?.source_config?.builder).toBe("railpack");
  });

  it("loads one service for the dedicated detail route", async () => {
    api.get.mockResolvedValueOnce({ success: true, data: service });
    const { useService } = await import("./useService");
    const services = useService();

    const loaded = await services.get(service.id);

    expect(loaded?.id).toBe(service.id);
    expect(services.data.value[0]?.id).toBe(service.id);
  });

  it("clears loading when the service request rejects", async () => {
    api.list.mockImplementationOnce(() => Promise.reject(new Error("network offline")));
    const { useService } = await import("./useService");
    const services = useService();

    await services.load("project-id");

    expect(services.loading.value).toBe(false);
    expect(services.error.value).toBe("network offline");
  });
});
