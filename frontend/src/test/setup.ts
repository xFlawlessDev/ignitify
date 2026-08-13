import { beforeEach, vi } from "vitest";

const blockedFetch: typeof fetch = async (input) => {
  const target = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
  throw new Error(`Unexpected network request in test: ${target}`);
};

beforeEach(() => {
  vi.stubGlobal("fetch", blockedFetch);
});
