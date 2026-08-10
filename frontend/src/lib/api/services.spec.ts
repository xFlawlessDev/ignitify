import { afterEach, describe, expect, it, vi } from "vitest";
import type { ServiceInput } from "../types";

const api = vi.hoisted(() => ({ fetch: vi.fn() }));

vi.mock("./core", () => ({ apiFetch: api.fetch }));

const gitComposeInput: ServiceInput = {
  name: "stack",
  kind: "compose",
  internal_port: 8080,
  healthcheck: null,
  variables: [],
  source_config: {
    source: "compose",
    provider_id: "provider-1",
    repository: "acme/stack",
    branch: "main",
  },
};

afterEach(() => {
  api.fetch.mockReset();
});

describe("apiUpdateService", () => {
  it("allows a Git Compose source without inline YAML", async () => {
    api.fetch.mockResolvedValue({ success: true, data: { id: "service-1" } });
    const { apiUpdateService } = await import("./services");

    await apiUpdateService("service-1", gitComposeInput);

    expect(api.fetch.mock.calls).toEqual([
      [
        "/services/service-1",
        {
          method: "PATCH",
          body: JSON.stringify(gitComposeInput),
        },
      ],
    ]);
  });

  it("still requires YAML for inline Compose", async () => {
    const { apiUpdateService } = await import("./services");

    const result = await apiUpdateService("service-1", {
      ...gitComposeInput,
      source_config: { source: "compose" },
    });

    expect(result.error).toBe("Compose YAML is required before saving the service.");
    expect(api.fetch).not.toHaveBeenCalled();
  });
});
