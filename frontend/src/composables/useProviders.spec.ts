// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  create: vi.fn(),
  list: vi.fn(),
  remove: vi.fn(),
  test: vi.fn(),
  update: vi.fn(),
}));

vi.mock("@/lib/api/providers", () => ({
  apiCreateProvider: api.create,
  apiDeleteProvider: api.remove,
  apiListProviders: api.list,
  apiTestProviderConnection: api.test,
  apiUpdateProvider: api.update,
}));

afterEach(() => {
  Object.values(api).forEach((mock) => mock.mockReset());
  vi.resetModules();
});

const provider = {
  id: "provider-1",
  name: "Main GitLab",
  kind: "gitlab" as const,
  auth_mode: "oauth" as const,
  base_url: "https://gitlab.example.com",
  internal_url: null,
  redirect_uri: "https://ignitify.example.com/callback",
  client_id: "client-id",
  application_id: null,
  installation_id: null,
  group_names: null,
  username: "deploy",
  token_configured: true,
  created_at: "2026-08-06T00:00:00Z",
  updated_at: "2026-08-06T00:00:00Z",
  last_verified_at: null,
};

describe("useProviders", () => {
  it("loads provider metadata and adds a created connection", async () => {
    api.list.mockResolvedValue({ success: true, data: [provider] });
    api.create.mockResolvedValue({ success: true, data: { ...provider, id: "provider-2" } });
    const { useProviders } = await import("./useProviders");
    const providers = useProviders();

    await providers.load();
    await providers.create({
      name: "Backup Gitea",
      kind: "gitea",
      auth_mode: "oauth",
      base_url: "https://gitea.example.com",
      token: "secret",
    });

    expect(providers.data.value.map((item) => item.id)).toEqual(["provider-2", "provider-1"]);
  });

  it("removes a connection only after the API succeeds", async () => {
    api.remove.mockResolvedValue({ success: true, data: undefined });
    const { useProviders } = await import("./useProviders");
    const providers = useProviders();
    providers.data.value = [provider];

    expect(await providers.remove(provider.id)).toBe(true);
    expect(providers.data.value).toEqual([]);
  });

  it("records a successful connection test timestamp and repository count", async () => {
    api.test.mockResolvedValue({
      success: true,
      data: { repository_count: 7, checked_at: "2026-08-06T12:00:00Z" },
    });
    const { useProviders } = await import("./useProviders");
    const providers = useProviders();
    providers.data.value = [provider];

    const result = await providers.testConnection(provider.id);
    expect(result).toEqual({
      repository_count: 7,
      checked_at: "2026-08-06T12:00:00Z",
    });
    expect(providers.data.value[0].last_verified_at).toBe("2026-08-06T12:00:00Z");
  });
});
