// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { isVersionNewer, useAppUpdate } from "./useAppUpdate";

describe("useAppUpdate", () => {
  it("reports an available release newer than the installed version", async () => {
    let requestedUrl: RequestInfo | URL | undefined;
    let requestedOptions: RequestInit | undefined;
    const fetchFn: typeof fetch = async (input, init) => {
      requestedUrl = input;
      requestedOptions = init;
      return new Response(
        JSON.stringify({ tag_name: "v0.2.0", html_url: "https://example.com/v0.2.0" }),
        { status: 200 },
      );
    };
    const update = useAppUpdate({ currentVersion: "0.1.0", fetchFn });

    const result = await update.checkForUpdate();

    const requestUrlText =
      typeof requestedUrl === "string"
        ? requestedUrl
        : requestedUrl instanceof URL
          ? requestedUrl.href
          : requestedUrl?.url;
    expect(requestUrlText).toContain("/releases/latest");
    expect(requestedOptions).toEqual({
      headers: { Accept: "application/vnd.github+json" },
    });
    expect(result).toEqual({
      kind: "updateAvailable",
      version: "0.2.0",
      releaseUrl: "https://example.com/v0.2.0",
    });
    expect(update.isChecking.value).toBeFalsy();
  });

  it("reports the installed version when the latest release is not newer", async () => {
    const fetchFn: typeof fetch = async () =>
      new Response(JSON.stringify({ tag_name: "v0.1.0", html_url: "https://example.com/v0.1.0" }), {
        status: 200,
      });
    const update = useAppUpdate({ currentVersion: "0.1.0", fetchFn });

    const result = await update.checkForUpdate();

    expect(result).toEqual({ kind: "upToDate", version: "0.1.0" });
  });

  it("handles repositories without a published release", async () => {
    const fetchFn: typeof fetch = async () => new Response(null, { status: 404 });
    const update = useAppUpdate({ currentVersion: "0.1.0", fetchFn });

    const result = await update.checkForUpdate();

    expect(result).toEqual({ kind: "noRelease" });
  });
});

describe("isVersionNewer", () => {
  it("compares prerelease versions according to semantic version precedence", () => {
    expect(isVersionNewer("1.0.0", "1.0.0-rc.1")).toBeTruthy();
    expect(isVersionNewer("1.0.0-rc.1", "1.0.0")).toBeFalsy();
    expect(isVersionNewer("1.0.0-beta.2", "1.0.0-beta.1")).toBeTruthy();
  });
});
