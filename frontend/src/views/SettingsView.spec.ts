// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";

const initialSettings = {
  server_domain: "",
  https_enabled: false,
  automatically_provision_ssl: false,
  certificate_provider: "none",
  custom_certificate_id: null,
  concurrent_builds: 2,
  certificates: [],
  updated_at: "2026-01-01T00:00:00Z",
};

async function settle() {
  for (let index = 0; index < 3; index += 1) {
    await Promise.resolve();
    await nextTick();
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

function requestUrl(input: RequestInfo | URL): string {
  return typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url;
}

async function mountSettings() {
  const component = (await import("./SettingsView.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component);
  app.mount(host);
  await settle();
  return { app, host };
}

afterEach(() => {
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("SettingsView", () => {
  let fetchCalls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];

  beforeEach(() => {
    fetchCalls = [];
    vi.stubGlobal(
      "fetch",
      vi.fn((input: RequestInfo | URL, init?: RequestInit) => {
        fetchCalls.push([input, init]);
        const url = requestUrl(input);
        const method = init?.method ?? "GET";
        if (url.endsWith("/settings/server") && method === "GET") {
          return Promise.resolve(
            new Response(JSON.stringify(initialSettings), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/server") && method === "PATCH") {
          const body = JSON.parse(typeof init?.body === "string" ? init.body : "{}");
          return Promise.resolve(
            new Response(JSON.stringify({ ...initialSettings, ...body }), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/server/certificates") && method === "POST") {
          return Promise.resolve(
            new Response(
              JSON.stringify({
                id: "certificate-1",
                name: "Production wildcard",
                certificate_file_name: "production.crt",
                private_key_file_name: "production.key",
                created_at: "2026-01-01T00:00:00Z",
                updated_at: "2026-01-01T00:00:00Z",
              }),
              { status: 201, headers: { "Content-Type": "application/json" } },
            ),
          );
        }
        if (url.includes("/settings/server/certificates/") && method === "DELETE") {
          return Promise.resolve(new Response(null, { status: 204 }));
        }
        return Promise.resolve(
          new Response(JSON.stringify({ error: "unexpected request" }), { status: 500 }),
        );
      }),
    );
  });

  it("loads and persists a valid server configuration through the API", async () => {
    const { app, host } = await mountSettings();
    const domain = host.querySelector("#server-domain") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    expect(save.disabled).toBe(true);

    domain.value = "control.example.com";
    domain.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(save.disabled).toBe(false);
    save.click();
    await settle();

    expect(host.textContent).toContain("Saved to server");
    expect(window.localStorage.length).toBe(0);
    const calls = fetchCalls;
    expect(
      calls.some(
        ([input, init]) =>
          requestUrl(input).endsWith("/settings/server") && init?.method === "PATCH",
      ),
    ).toBe(true);
    app.unmount();
  });

  it("rejects a concurrent build count outside the supported range", async () => {
    const { app, host } = await mountSettings();
    const domain = host.querySelector("#server-domain") as HTMLInputElement;
    const builds = host.querySelector("#concurrent-builds") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    domain.value = "control.example.com";
    domain.dispatchEvent(new Event("input", { bubbles: true }));
    builds.value = "33";
    builds.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(host.textContent).toContain("Use no more than 32 concurrent builds.");
    expect(save.disabled).toBe(true);
    app.unmount();
  });

  it("uploads a custom certificate pair through the server API", async () => {
    const { app, host } = await mountSettings();
    const addCertificate = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Add certificate"),
    ) as HTMLButtonElement;

    addCertificate.click();
    await nextTick();

    const name = document.querySelector("#certificate-name") as HTMLInputElement;
    const certificateFile = document.querySelector("#certificate-file") as HTMLInputElement;
    const privateKeyFile = document.querySelector("#private-key-file") as HTMLInputElement;
    name.value = "Production wildcard";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    Object.defineProperty(certificateFile, "files", {
      configurable: true,
      value: [new File(["certificate"], "production.crt")],
    });
    certificateFile.dispatchEvent(new Event("change", { bubbles: true }));
    Object.defineProperty(privateKeyFile, "files", {
      configurable: true,
      value: [new File(["private-key"], "production.key")],
    });
    privateKeyFile.dispatchEvent(new Event("change", { bubbles: true }));
    await nextTick();

    const form = document.querySelector('[data-slot="dialog-content"] form') as HTMLFormElement;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await settle();

    expect(host.textContent).toContain("Production wildcard");
    expect(host.textContent).toContain("production.crt");
    expect(host.textContent).toContain("production.key");
    app.unmount();
  });
});
