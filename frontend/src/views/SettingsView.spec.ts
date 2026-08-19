// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

const mocks = vi.hoisted(() => ({
  toastSuccess: vi.fn(),
  getInfrastructure: vi.fn(),
  updateInfrastructure: vi.fn(),
  createCertificate: vi.fn(),
  deleteCertificate: vi.fn(),
  getBackupDestination: vi.fn(),
  updateBackupDestination: vi.fn(),
  updateBackupControls: vi.fn(),
  deleteBackupDestination: vi.fn(),
  listBackupRuns: vi.fn(),
  getOperationalHealth: vi.fn(),
}));

vi.mock("vue-sonner", () => ({
  toast: {
    error: vi.fn(),
    success: mocks.toastSuccess,
  },
}));

vi.mock("@/lib/api", () => ({
  apiCreateInfrastructureCertificate: mocks.createCertificate,
  apiDeleteInfrastructureCertificate: mocks.deleteCertificate,
  apiGetInfrastructureSettings: mocks.getInfrastructure,
  apiUpdateInfrastructureSettings: mocks.updateInfrastructure,
  apiDeleteBackupS3Destination: mocks.deleteBackupDestination,
  apiGetBackupS3Destination: mocks.getBackupDestination,
  apiListBackupS3Runs: mocks.listBackupRuns,
  apiGetOperationalHealthSummary: mocks.getOperationalHealth,
  apiUpdateBackupS3Controls: mocks.updateBackupControls,
  apiUpdateBackupS3Destination: mocks.updateBackupDestination,
}));

const mountedApps: Array<{ unmount: () => void }> = [];

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

async function mountSettings() {
  const component = (await import("./SettingsView.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component);
  app.use(i18n);
  app.mount(host);
  mountedApps.push(app);
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
  for (const app of mountedApps) app.unmount();
  mountedApps.length = 0;
  document.body.replaceChildren();
  for (const mock of Object.values(mocks)) mock.mockReset();
});

describe("SettingsView", () => {
  let backupDestination: Record<string, unknown> | null = null;

  beforeEach(() => {
    backupDestination = null;
    mocks.getInfrastructure.mockResolvedValue({ success: true, data: initialSettings });
    mocks.updateInfrastructure.mockImplementation((...args) => {
      const input = args[0] as Record<string, unknown>;
      return Promise.resolve({ success: true, data: { ...initialSettings, ...input } });
    });
    mocks.createCertificate.mockResolvedValue({
      success: true,
      data: {
        id: "certificate-1",
        name: "Production wildcard",
        certificate_file_name: "production.crt",
        private_key_file_name: "production.key",
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      },
    });
    mocks.deleteCertificate.mockResolvedValue({ success: true, data: null });
    mocks.getBackupDestination.mockImplementation(() =>
      Promise.resolve({ success: true, data: backupDestination }),
    );
    mocks.updateBackupDestination.mockImplementation((...args) => {
      backupDestination = {
        ...(args[0] as Record<string, unknown>),
        created_at: "2026-01-01T00:00:00Z",
        updated_at: "2026-01-01T00:00:00Z",
      };
      return Promise.resolve({ success: true, data: backupDestination });
    });
    mocks.updateBackupControls.mockImplementation((...args) => {
      backupDestination = { ...backupDestination, ...(args[0] as Record<string, unknown>) };
      return Promise.resolve({ success: true, data: backupDestination });
    });
    mocks.deleteBackupDestination.mockResolvedValue({ success: true, data: null });
    mocks.listBackupRuns.mockResolvedValue({
      success: true,
      data: [
        {
          id: "backup-1",
          trigger: "scheduled",
          status: "succeeded",
          started_at: "2026-01-01T00:00:00Z",
          completed_at: "2026-01-01T00:03:00Z",
          message: "Backup completed",
        },
      ],
    });
    mocks.getOperationalHealth.mockResolvedValue({
      success: true,
      data: {
        generated_at: "2026-01-01T00:00:00Z",
        control_plane: { status: "ready" },
        runtime: { status: "ready" },
        worker: { status: "ready" },
        ingress: { status: "ready" },
        deployments: {
          status: "healthy",
          queued_count: 0,
          active_count: 0,
          failed_count: 0,
          failed_retry_count: 0,
          recent_failed_retry_count: 0,
          retry_count: 0,
          average_duration_seconds: null,
          latest_duration_seconds: null,
        },
        backup: {
          status: "not_configured",
          configured: false,
          enabled: false,
          schedule_interval_hours: null,
          latest_status: null,
          latest_started_at: null,
          latest_completed_at: null,
          latest_age_seconds: null,
        },
        domains: { status: "healthy", active_count: 0, pending_count: 0, failed_count: 0 },
        certificates: {
          status: "healthy",
          https_enabled: true,
          provider: "lets-encrypt",
          custom_certificate_selected: false,
          stored_certificate_count: 0,
        },
        remote_agents: {
          status: "not_configured",
          server_count: 0,
          online_count: 0,
          offline_count: 0,
          pending_count: 0,
          oldest_heartbeat_at: null,
          oldest_heartbeat_age_seconds: null,
        },
      },
    });
  });

  it("loads infrastructure health and persists an application ingress policy", async () => {
    const { host } = await mountSettings();
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    expect(save.disabled).toBe(true);

    expect(host.textContent).toContain("Control plane health");
    expect(host.textContent).toContain("Traefik");
    expect(host.textContent).toContain("Application environment");
    expect(host.textContent).toContain("Build capacity");
    expect(host.textContent).toContain("Health summary");
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
    expect(mocks.updateInfrastructure).toHaveBeenCalled();
  });

  it("rejects an invalid application domain suffix", async () => {
    const { host } = await mountSettings();
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
  });

  it("configures a separate HTTPS control-plane domain", async () => {
    const { host } = await mountSettings();
    await selectSection(host, "Ingress & TLS");
    const applicationDomain = host.querySelector("#application-domain-suffix") as HTMLInputElement;
    const controlPlaneDomain = host.querySelector("#control-plane-domain") as HTMLInputElement;
    const save = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Save changes"),
    ) as HTMLButtonElement;

    applicationDomain.value = "apps.example.com";
    applicationDomain.dispatchEvent(new Event("input", { bubbles: true }));
    controlPlaneDomain.value = "console.apps.example.com";
    controlPlaneDomain.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    expect(save.disabled).toBe(false);
    expect(host.textContent).toContain("console.apps.example.com");
    expect(host.textContent).not.toContain("admin.example.com");
    save.click();
    await settle();

    const payload = mocks.updateInfrastructure.mock.calls[0]?.[0] as
      | Record<string, unknown>
      | undefined;
    expect(payload?.control_plane_domain).toBe("console.apps.example.com");
  });

  it("shows the current Cloudflare Tunnel environment requirements", async () => {
    mocks.getInfrastructure.mockResolvedValue({
      success: true,
      data: {
        ...initialSettings,
        control_plane_domain: "admin.example.com",
        application_domain_suffix: "apps.example.com",
        dns_record_type: "cname",
        dns_record_target: "tunnel-id.cfargotunnel.com",
      },
    });

    const { host } = await mountSettings();
    await selectSection(host, "Ingress & TLS");

    expect(host.textContent).toContain("*.apps.example.com");
    expect(host.textContent).toContain("IGNITIFY_REMOTE_MODE=true");
    expect(host.textContent).toContain("IGNITIFY_TRUSTED_ORIGINS=https://admin.example.com");
    expect(host.textContent).toContain("IGNITIFY_SECURE_COOKIES=true");
    expect(host.textContent).not.toContain("IGNITIFY_TRUST_PROXY_HEADERS=true");
  });

  it("persists a custom unmatched-hostname page", async () => {
    const { host } = await mountSettings();
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

    const payload = mocks.updateInfrastructure.mock.calls[0]?.[0] as
      | Record<string, unknown>
      | undefined;
    expect(payload?.fallback_page_heading).toBe("This site is not deployed");
    expect(payload?.fallback_page_message).toBe("Check the domain name and try again.");
  });

  it("uploads a custom certificate pair through the server API", async () => {
    const { host } = await mountSettings();
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
  });

  it("stores a write-only S3 backup destination separately from infrastructure settings", async () => {
    const { host } = await mountSettings();
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

    const payload = mocks.updateBackupDestination.mock.calls[0]?.[0] as
      | Record<string, unknown>
      | undefined;
    expect(payload).toMatchObject({
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

    const controls = mocks.updateBackupControls.mock.calls[0]?.[0];
    expect(controls).toEqual({
      enabled: true,
      schedule_interval_hours: 48,
    });
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
    const { host } = await mountSettings();
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

    const controls = mocks.updateBackupControls.mock.calls[0]?.[0];
    expect(controls).toEqual({
      enabled: false,
      schedule_interval_hours: null,
    });
    expect(host.textContent).toContain("disabled");
  });
});
