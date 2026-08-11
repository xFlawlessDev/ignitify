// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

const mocks = vi.hoisted(() => ({ toastSuccess: vi.fn() }));

vi.mock("vue-sonner", () => ({
  toast: {
    error: vi.fn(),
    success: mocks.toastSuccess,
  },
}));

const initialSettings = {
  application: {
    public_origin: "http://127.0.0.1:6565",
    secure_cookies: false,
  },
  control_plane_domain: "",
  application_domain_suffix: "",
  https_enabled: true,
  automatically_provision_ssl: true,
  acme_email: "ops@example.com",
  dns_record_type: "a" as const,
  dns_record_target: "",
  fallback_page_heading: "Application not found",
  fallback_page_message: "The requested hostname is not connected to an active application.",
  certificate_provider: "lets-encrypt",
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
  app.use(i18n);
  app.mount(host);
  await settle();
  return { app, host };
}

async function selectSection(host: HTMLElement, label: string) {
  const tab = [...host.querySelectorAll('[role="tab"]')].find((element) =>
    element.textContent?.includes(label),
  );

  if (!(tab instanceof HTMLButtonElement)) {
    throw new Error(`Could not find the ${label} settings tab.`);
  }

  tab.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, button: 0 }));
  await settle();
}

afterEach(() => {
  document.body.replaceChildren();
  vi.unstubAllGlobals();
  vi.resetModules();
  mocks.toastSuccess.mockReset();
});

describe("SettingsView", () => {
  let fetchCalls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  let backupDestination: Record<string, unknown> | null = null;

  beforeEach(() => {
    fetchCalls = [];
    backupDestination = null;
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
        if (url.endsWith("/settings/backup-destination/s3") && method === "GET") {
          return Promise.resolve(
            new Response(JSON.stringify(backupDestination), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/backup-destination/s3") && method === "PUT") {
          const body = JSON.parse(typeof init?.body === "string" ? init.body : "{}");
          backupDestination = {
            ...body,
            server_side_encryption: body.server_side_encryption,
            created_at: "2026-01-01T00:00:00Z",
            updated_at: "2026-01-01T00:00:00Z",
          };
          return Promise.resolve(
            new Response(JSON.stringify(backupDestination), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/backup-destination/s3") && method === "PATCH") {
          const body = JSON.parse(typeof init?.body === "string" ? init.body : "{}");
          backupDestination = { ...backupDestination, ...body };
          return Promise.resolve(
            new Response(JSON.stringify(backupDestination), {
              status: 200,
              headers: { "Content-Type": "application/json" },
            }),
          );
        }
        if (url.endsWith("/settings/backup-destination/s3/runs") && method === "GET") {
          return Promise.resolve(
            new Response(
              JSON.stringify([
                {
                  id: "backup-1",
                  trigger: "scheduled",
                  status: "succeeded",
                  started_at: "2026-01-01T00:00:00Z",
                  completed_at: "2026-01-01T00:03:00Z",
                  message: "Backup completed",
                },
              ]),
              { status: 200, headers: { "Content-Type": "application/json" } },
            ),
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
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    expect(save.disabled).toBe(true);

    expect(host.textContent).toContain("Control plane health");
    expect(host.textContent).toContain("Traefik");
    expect(host.textContent).toContain("Application environment");
    expect(host.textContent).toContain("Build capacity");
    expect(host.textContent).toContain("http://127.0.0.1:6565");
    await selectSection(host, "Ingress & TLS");
    const domain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    expect(host.textContent).toContain("DNS record type");
    expect(host.querySelector("#dns-record-target")).not.toBeNull();
    expect(host.textContent).toContain("Create a separate A record for the control plane:");
    expect(host.textContent).toContain(
      "Saving the control-plane hostname enables its HTTPS trusted origin",
    );

    domain.value = "apps.example.com";
    domain.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(save.disabled).toBe(false);
    save.click();
    await settle();

    expect(mocks.toastSuccess.mock.calls).toEqual([["Infrastructure settings saved"]]);
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
    await selectSection(host, "Ingress & TLS");
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

  it("configures a separate HTTPS control-plane domain", async () => {
    const { app, host } = await mountSettings();
    await selectSection(host, "Ingress & TLS");
    const applicationDomain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    const controlPlaneDomain = host.querySelector("#control-plane-domain") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    applicationDomain.value = "apps.example.com";
    applicationDomain.dispatchEvent(new Event("input", { bubbles: true }));
    controlPlaneDomain.value = "console.example.com";
    controlPlaneDomain.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(save.disabled).toBe(false);
    expect(host.textContent).toContain("console.example.com");
    expect(host.textContent).not.toContain("admin.example.com");
    save.click();
    await settle();

    const patch = fetchCalls.find(
      ([input, init]) =>
        requestUrl(input).endsWith("/settings/infrastructure") && init?.method === "PATCH",
    );
    const requestBody = patch?.[1]?.body;
    if (typeof requestBody !== "string") throw new Error("Expected a JSON settings payload.");
    expect(JSON.parse(requestBody).control_plane_domain).toBe("console.example.com");
    app.unmount();
  });

  it("persists a custom unmatched-hostname page", async () => {
    const { app, host } = await mountSettings();
    await selectSection(host, "Ingress & TLS");
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
    await selectSection(host, "Ingress & TLS");
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

  it("stores a write-only S3 backup destination separately from infrastructure settings", async () => {
    const { app, host } = await mountSettings();
    await selectSection(host, "Backup");
    const endpoint = host.querySelector("#s3-backup-endpoint") as HTMLInputElement;
    const bucket = host.querySelector("#s3-backup-bucket") as HTMLInputElement;
    const accessKey = host.querySelector("#s3-access-key-id") as HTMLInputElement;
    const secretKey = host.querySelector("#s3-secret-access-key") as HTMLInputElement;
    const scheduler = host.querySelector("#draft-backup-scheduler") as HTMLButtonElement;

    endpoint.value = "https://account.r2.cloudflarestorage.com";
    endpoint.dispatchEvent(new Event("input", { bubbles: true }));
    bucket.value = "ignitify-backups";
    bucket.dispatchEvent(new Event("input", { bubbles: true }));
    accessKey.value = "access-key-id";
    accessKey.dispatchEvent(new Event("input", { bubbles: true }));
    secretKey.value = "secret-access-key";
    secretKey.dispatchEvent(new Event("input", { bubbles: true }));
    scheduler.click();
    await nextTick();
    const scheduleInterval = host.querySelector(
      "#draft-backup-schedule-interval",
    ) as HTMLInputElement;
    scheduleInterval.value = "48";
    scheduleInterval.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    const form = endpoint.closest("form") as HTMLFormElement;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await settle();

    const request = fetchCalls.find(
      ([input, init]) =>
        requestUrl(input).endsWith("/settings/backup-destination/s3") && init?.method === "PUT",
    );
    const body = request?.[1]?.body;
    if (typeof body !== "string") throw new Error("Expected a JSON S3 destination payload.");
    expect(JSON.parse(body)).toMatchObject({
      endpoint: "https://account.r2.cloudflarestorage.com",
      bucket: "ignitify-backups",
      access_key_id: "access-key-id",
      secret_access_key: "secret-access-key",
      enabled: false,
      schedule_interval_hours: 48,
    });
    expect(host.textContent).toContain("disabled");
    expect(host.textContent).toContain("Backup completed");

    const backupEnabled = host.querySelector("#backup-enabled") as HTMLButtonElement;
    backupEnabled.click();
    await settle();

    const controlsRequest = fetchCalls.find(
      ([input, init]) =>
        requestUrl(input).endsWith("/settings/backup-destination/s3") && init?.method === "PATCH",
    );
    const controlsBody = controlsRequest?.[1]?.body;
    if (typeof controlsBody !== "string")
      throw new Error("Expected a JSON backup controls payload.");
    expect(JSON.parse(controlsBody)).toEqual({
      enabled: true,
      schedule_interval_hours: 48,
    });
    app.unmount();
  });

  it("disables an existing destination without requiring replacement credentials", async () => {
    backupDestination = {
      endpoint: "https://account.r2.cloudflarestorage.com",
      region: "us-east-1",
      bucket: "ignitify-backups",
      prefix: "ignitify",
      server_side_encryption: "AES256",
      enabled: true,
      schedule_interval_hours: null,
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
    const { app, host } = await mountSettings();
    await selectSection(host, "Backup");
    const replaceCredentials = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Replace credentials"),
    ) as HTMLButtonElement;

    replaceCredentials.click();
    await nextTick();
    const accessKey = host.querySelector("#s3-access-key-id") as HTMLInputElement;
    const backupEnabled = host.querySelector("#draft-backup-enabled") as HTMLButtonElement;
    expect(accessKey.value).toBe("");

    backupEnabled.click();
    await nextTick();
    expect(host.textContent).not.toContain("Enter an S3 access key ID.");
    const form = accessKey.closest("form") as HTMLFormElement;
    form.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await settle();

    const request = fetchCalls.find(
      ([input, init]) =>
        requestUrl(input).endsWith("/settings/backup-destination/s3") && init?.method === "PATCH",
    );
    const body = request?.[1]?.body;
    if (typeof body !== "string") throw new Error("Expected a JSON backup controls payload.");
    expect(JSON.parse(body)).toEqual({
      enabled: false,
      schedule_interval_hours: null,
    });
    expect(host.textContent).toContain("disabled");
    app.unmount();
  });
});
