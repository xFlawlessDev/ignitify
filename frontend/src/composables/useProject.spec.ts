// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  get: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@/lib/api/projects", () => ({
  apiGetProject: api.get,
  apiUpdateProject: api.update,
}));

const project = (id: string) => ({
  id,
  name: `Project ${id}`,
  owner_id: id,
  role: "owner" as const,
  created_at: "2026-07-31T00:00:00Z",
  updated_at: "2026-07-31T00:00:00Z",
  default_environment: {
    id,
    name: "production",
    is_default: true,
  },
});

afterEach(() => {
  api.get.mockReset();
  api.update.mockReset();
  vi.resetModules();
});

describe("useProject", () => {
  it("keeps latest project when earlier load resolves late", async () => {
    let resolveFirst: (value: unknown) => void;
    api.get.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    );
    api.get.mockResolvedValueOnce({ success: true, data: project("second") });
    const { useProject } = await import("./useProject");
    const currentProject = useProject();

    const first = currentProject.load("first");
    await currentProject.load("second");
    resolveFirst!({ success: true, data: project("first") });
    await first;

    expect(currentProject.data.value?.id).toBe("second");
  });
});
