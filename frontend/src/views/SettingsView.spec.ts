// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";

const initialSettings = {
  application: {
    public_origin: "http://127.0.0.1:6565",
    secure_cookies: false,
  },
  application_domain_suffix: "",
  https_enabled: false,
  automatically_provision_ssl: false,
  acme_email: "",
  dns_record_type: "a" as const,
  dns_record_target: "",
  fallback_page_heading: "Application not found",
  fallback_page_message: "The requested hostname is not connected to an active application.",
  certificate_provider: "none",
  custom_certificate_id: null,
  concurrent_builds: 2,
  certificates: [],
  health: {
    database: "ready",
    runtime: "ready",
    worker: "ready",
    ingress: "ready",
  },
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
        if (url.endsWith("/settings/infrastructure") && method === "GET") {
          return Promise.resolve(
            new Response(JSON.stringify(initialSettings), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/infrastructure") && method === "PATCH") {
          const body = JSON.parse(typeof init?.body === "string" ? init.body : "{}");
          return Promise.resolve(
            new Response(JSON.stringify({ ...initialSettings, ...body }), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/infrastructure/certificates") && method === "POST") {
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
        if (url.includes("/settings/infrastructure/certificates/") && method === "DELETE") {
          return Promise.resolve(new Response(null, { status: 204 }));
        }
        return Promise.resolve(
          new Response(JSON.stringify({ error: "unexpected request" }), { status: 500 }),
        );
      }),
    );
  });

  it("loads infrastructure health and persists an application ingress policy", async () => {
    const { app, host } = await mountSettings();
    const domain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    expect(save.disabled).toBe(true);

    expect(host.textContent).toContain("Control plane health");
    expect(host.textContent).toContain("Traefik");
    expect(host.textContent).toContain("Application environment");
    expect(host.textContent).toContain("DNS record type");
    expect(host.querySelector("#dns-record-target")).not.toBeNull();
    expect(host.textContent).toContain("http://127.0.0.1:6565");

    domain.value = "apps.example.com";
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
          requestUrl(input).endsWith("/settings/infrastructure") && init?.method === "PATCH",
      ),
    ).toBe(true);
    app.unmount();
  });

  it("rejects an invalid application domain suffix", async () => {
    const { app, host } = await mountSettings();
    const domain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    domain.value = "https://apps.example.com";
    domain.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(host.textContent).toContain("Use a valid hostname without a protocol or path.");
    expect(save.disabled).toBe(true);
    app.unmount();
  });

  it("persists a custom unmatched-hostname page", async () => {
    const { app, host } = await mountSettings();
    const domain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    const heading = host.querySelector("#fallback-page-heading") as HTMLInputElement;
    const message = host.querySelector("#fallback-page-message") as HTMLTextAreaElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    domain.value = "apps.example.com";
    domain.dispatchEvent(new Event("input", { bubbles: true }));
    heading.value = "This site is not deployed";
    heading.dispatchEvent(new Event("input", { bubbles: true }));
    message.value = "Check the domain name and try again.";
    message.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(save.disabled).toBe(false);
    save.click();
    await settle();

    const patch = fetchCalls.find(
      ([input, init]) =>
        requestUrl(input).endsWith("/settings/infrastructure") && init?.method === "PATCH",
    );
    const requestBody = patch?.[1]?.body;
    if (typeof requestBody !== "string") throw new Error("Expected a JSON settings payload.");
    const body = JSON.parse(requestBody);
    expect(body.fallback_page_heading).toBe("This site is not deployed");
    expect(body.fallback_page_message).toBe("Check the domain name and try again.");
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
