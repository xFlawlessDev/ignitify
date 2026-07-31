// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  list: vi.fn(),
}));

vi.mock("@/lib/api/projects", () => ({
  apiCreateProject: api.create,
  apiListProjects: api.list,
}));

afterEach(() => {
  api.create.mockReset();
  api.list.mockReset();
  vi.resetModules();
});

describe("useProjects", () => {
  it("exposes loading then empty data", async () => {
    let resolveList: (value: unknown) => void;
    api.list.mockReturnValue(
      new Promise((resolve) => {
        resolveList = resolve;
      }),
    );
    const { useProjects } = await import("./useProjects");
    const projects = useProjects();

    const loading = projects.load();
    expect(projects.loading.value).toBeTruthy();
    resolveList!({ success: true, data: [] });
    await loading;

    expect(projects.data.value).toEqual([]);
  });

  it("exposes list request error", async () => {
    api.list.mockResolvedValue({ success: false, data: [], error: "offline" });
    const { useProjects } = await import("./useProjects");
    const projects = useProjects();

    await projects.load();

    expect(projects.error.value).toBe("offline");
  });

  it("adds created project to data", async () => {
    api.create.mockResolvedValue({
      success: true,
      data: {
        id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
        name: "Platform",
        owner_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
        role: "owner",
        created_at: "2026-07-31T00:00:00Z",
        updated_at: "2026-07-31T00:00:00Z",
        default_environment: {
          id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
          name: "production",
          is_default: true,
        },
      },
    });
    const { useProjects } = await import("./useProjects");
    const projects = useProjects();

    await projects.create({ name: "Platform" });

    expect(projects.data.value[0]?.name).toBe("Platform");
  });
});
