// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

import { apiFetchRaw, clearToken, setToken } from "./core";

afterEach(() => {
  clearToken();
  vi.unstubAllGlobals();
});

describe("apiFetchRaw", () => {
  it("rejects cross-origin API URLs before issuing a request", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    setToken("access-token");

    let message = "";
    try {
      await apiFetchRaw("https://untrusted.example/api/v1/projects");
    } catch (error) {
      message = error instanceof Error ? error.message : "";
    }

    expect(message).toBe("API requests must target the current origin");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("sends authorization and state-change protection only to the application origin", async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);
    setToken("access-token");

    await apiFetchRaw("/projects", { method: "POST" });

    const options = fetchMock.mock.calls[0]?.[1] as RequestInit;
    const headers = new Headers(options.headers);
    expect(headers.get("Authorization")).toBe("Bearer access-token");
    expect(headers.get("X-Ignitify-Request")).toBe("1");
    expect(headers.get("X-Ignitify-Request-ID")).toBeTruthy();
    expect(options.credentials).toBe("same-origin");
  });
});
