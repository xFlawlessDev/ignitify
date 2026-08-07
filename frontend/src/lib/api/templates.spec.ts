import { afterEach, describe, expect, it, vi } from "vitest";

import { apiListTemplates } from "./templates";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("apiListTemplates", () => {
  it("requests a server-filtered catalog page and returns its metadata", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(
        JSON.stringify({
          items: [],
          page: 2,
          pageSize: 24,
          total: 56,
          totalPages: 3,
          hasNextPage: true,
          hasPreviousPage: true,
        }),
        { status: 200 },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    const result = await apiListTemplates({
      page: 2,
      pageSize: 24,
      query: "database",
      tag: "database",
    });

    expect(fetchMock).toHaveBeenCalled();
    const firstCall = fetchMock.mock.calls[0];
    if (!firstCall) throw new Error("Expected a catalog request");

    expect(firstCall[0]).toBe(
      "http://localhost:4545/api/templates?page=2&page_size=24&q=database&tag=database",
    );
    expect((firstCall[1] as RequestInit).credentials).toBe("omit");
    expect(result).toEqual({
      success: true,
      data: {
        items: [],
        page: 2,
        pageSize: 24,
        total: 56,
        totalPages: 3,
        hasNextPage: true,
        hasPreviousPage: true,
      },
    });
  });
});
