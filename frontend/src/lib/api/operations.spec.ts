import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ fetch: vi.fn() }));

vi.mock("./core", () => ({ apiFetch: api.fetch }));

afterEach(() => {
  api.fetch.mockReset();
});

describe("apiGetOperationalHealthSummary", () => {
  it("requests the operator-only health summary endpoint", async () => {
    api.fetch.mockResolvedValue({ success: true, data: {} });
    const { apiGetOperationalHealthSummary } = await import("./operations");

    await apiGetOperationalHealthSummary();

    expect(api.fetch.mock.calls).toEqual([["/operations/health-summary"]]);
  });
});
