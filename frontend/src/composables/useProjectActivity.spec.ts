// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ list: vi.fn() }));

vi.mock("@/lib/api/activity", () => ({
  apiListProjectActivity: api.list,
}));

afterEach(() => {
  api.list.mockReset();
  vi.resetModules();
});

describe("useProjectActivity", () => {
  it("clears loading when activity loading throws", async () => {
    api.list.mockImplementationOnce(() => Promise.reject(new Error("activity unavailable")));
    const { useProjectActivity } = await import("./useProjectActivity");
    const activity = useProjectActivity();

    await activity.load("project-id");

    expect(activity.loading.value).toBe(false);
    expect(activity.error.value).toBe("activity unavailable");
  });
});
