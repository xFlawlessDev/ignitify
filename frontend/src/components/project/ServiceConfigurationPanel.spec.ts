// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";
import type { ServiceInput } from "@/lib/types";

const providerApi = vi.hoisted(() => ({
  repositories: vi.fn(),
  branches: vi.fn(),
}));

const templateApi = vi.hoisted(() => ({
  list: vi.fn(),
}));

vi.mock("@/lib/api/providers", () => ({
  apiListProviderRepositories: providerApi.repositories,
  apiListProviderBranches: providerApi.branches,
}));

vi.mock("@/lib/api/templates", () => ({
  apiListTemplates: templateApi.list,
  TEMPLATES_URL: "http://localhost:4545/api/templates",
}));

vi.mock("@/components/ui/tooltip", () => ({
  Tooltip: { template: "<div><slot /></div>" },
  TooltipContent: { template: "<span><slot /></span>" },
  TooltipTrigger: { template: "<span><slot /></span>" },
}));

const service = {
  id: "service-1",
  project_id: "project-1",
  environment_id: "environment-1",
  role: "owner" as const,
  name: "web",
  kind: "image" as const,
  image_reference:
    "caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a",
  internal_port: 80,
  healthcheck: null,
  desired_generation: 1,
  desired_state: "stopped" as const,
  created_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  variables: [],
};

afterEach(() => {
  document.body.replaceChildren();
  providerApi.repositories.mockReset();
  providerApi.branches.mockReset();
  vi.unstubAllGlobals();
  vi.resetModules();
});

beforeEach(() => {
  templateApi.list.mockResolvedValue({ success: true, data: [] });
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({ status: 404, ok: false }));
});

async function selectOption(host: HTMLElement, triggerId: string, label: string) {
  const trigger = host.querySelector(`#${triggerId}`) as HTMLButtonElement;
  trigger.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
  await nextTick();
  const option = [...document.body.querySelectorAll('[data-slot="select-item"]')].find((item) =>
    item.textContent?.includes(label),
  ) as HTMLElement | undefined;
  expect(option).toBeDefined();
  option?.dispatchEvent(new MouseEvent("pointerup", { bubbles: true, button: 0 }));
  await nextTick();
}

describe("ServiceConfigurationPanel", () => {
  it("preserves a stored secret when saving unrelated configuration changes", async () => {
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: {
        ...service,
        source_config: {
          source: "application",
          provider_id: "provider-1",
          repository: "acme/site",
          branch: "main",
          builder: "railpack",
        },
        variables: [{ key: "API_TOKEN", is_secret: true, is_set: true }],
      },
      providers: [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      variables: [{ key: "API_TOKEN", value: "", is_secret: true, preserve: true }],
    });
    app.unmount();
  });

  it("saves the selected remote deployment destination", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => [
          {
            id: "destination-1",
            name: "Production VM",
            host: "production.example.com",
            port: 22,
            username: "ignitify",
            deploy_path: "/srv/ignitify",
            private_key_configured: true,
            public_key_configured: true,
            known_hosts_configured: true,
            agent: null,
            is_default: true,
            created_at: "2026-08-01T00:00:00Z",
            updated_at: "2026-08-01T00:00:00Z",
          },
        ],
      }),
    );
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const remoteService = {
      ...service,
      kind: "compose" as const,
      compose_yaml:
        "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
      exposed_service: "web",
      source_config: { source: "compose" },
    };
    const app = createApp(component, {
      service: remoteService,
      providers: [],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    await selectOption(host, "service-config-destination", "Production VM");
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      deployment_destination_id: "destination-1",
    });
    app.unmount();
  });

  it("persists auto deploy and exposes the provider webhook setup", async () => {
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const onRotateAutoDeploySecret = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: {
        ...service,
        source_config: {
          source: "application",
          provider_id: "provider-1",
          repository: "acme/site",
          branch: "main",
          builder: "railpack",
          auto_deploy: true,
        },
        auto_deploy_webhook_secret: "generated-webhook-secret",
      },
      providers: [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
      onRotateAutoDeploySecret,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Auto deploy on push");
    expect(host.querySelector("#service-config-webhook-url")).not.toBeNull();
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      source_config: { auto_deploy: true },
    });
    const rotate = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Rotate secret"),
    ) as HTMLButtonElement;
    rotate.click();
    expect(onRotateAutoDeploySecret).toHaveBeenCalledTimes(1);
    app.unmount();
  });

  it("saves inline Compose YAML entered in the editor", async () => {
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      providers: [],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    const composeButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Compose"),
    ) as HTMLButtonElement;
    composeButton.click();
    await nextTick();
    expect(host.textContent).toContain("Compose requirements");
    expect(host.textContent).toContain("Application");
    const yaml = host.querySelector("#service-config-compose-yaml") as HTMLTextAreaElement;
    yaml.value = "services:\n  web:\n    image: nginx:1.27\n";
    yaml.dispatchEvent(new Event("input", { bubbles: true }));
    const exposedService = host.querySelector("#service-config-exposed") as HTMLInputElement;
    exposedService.value = "web";
    exposedService.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      kind: "compose",
      compose_yaml: "services:\n  web:\n    image: nginx:1.27\n",
      exposed_service: "web",
      source_config: { source: "compose" },
    });
    app.unmount();
  });

  it("explains Git Compose ingress selection", async () => {
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: {
        ...service,
        kind: "compose" as const,
        compose_yaml:
          "services:\n  web:\n    image: nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n",
        exposed_service: "web",
        source_config: {
          source: "compose",
          provider_id: "provider-1",
          repository: "acme/stack",
          branch: "main",
        },
      },
      providers: [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      inheritedVariables: [],
      saving: false,
      error: null,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("first service in the repository Compose file");
    app.unmount();
  });

  it("saves an application builder source inline", async () => {
    providerApi.repositories.mockResolvedValue({
      success: true,
      data: [{ name: "site", path: "acme/site", default_branch: "main" }],
    });
    providerApi.branches.mockResolvedValue({
      success: true,
      data: [{ name: "main" }, { name: "production" }],
    });
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      providers: [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    const applicationButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Application"),
    ) as HTMLButtonElement;
    applicationButton.click();
    await nextTick();
    await selectOption(host, "service-config-provider", "GitHub (github)");
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    await selectOption(host, "service-config-repository", "acme/site");
    await nextTick();
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      source_config: {
        source: "application",
        provider_id: "provider-1",
        repository: "acme/site",
        branch: "main",
        builder: "static",
      },
    });
    const saved = onSave.mock.calls[0]?.[0] as ServiceInput | undefined;
    expect(saved?.image_reference).toBeUndefined();
    expect(host.querySelector("#service-config-image")).toBeNull();
    app.unmount();
  });

  it("applies a remote template compose and saves its template source", async () => {
    templateApi.list.mockResolvedValue({
      success: true,
      data: [
        {
          id: "wordpress",
          name: "Wordpress",
          version: "latest",
          description: "A self-hosted CMS",
          tags: ["cms"],
        },
      ],
    });
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: string | URL) => {
        const url = String(input);
        if (url.endsWith("/docker-compose.yml")) {
          return {
            status: 200,
            ok: true,
            text: async () => "services:\n  wordpress:\n    image: wordpress:latest\n",
          };
        }
        if (url.endsWith("/template.toml")) {
          return {
            status: 200,
            ok: true,
            text: async () => `
[variables]
site_name = "Ignitify"
admin_password = "\${password:32}"

[config]
env = [
  "SITE_NAME=\${site_name}",
  "ADMIN_PASSWORD=\${admin_password}",
]

[[config.domains]]
serviceName = "wordpress"
port = 80
`,
          };
        }
        return { status: 404, ok: false, text: async () => "" };
      }),
    );
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      inheritedVariables: [],
      providers: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();

    expect(host.textContent).not.toContain("Wordpress");
    const chooseTemplateButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Choose template"),
    ) as HTMLButtonElement;
    expect(chooseTemplateButton).toBeDefined();
    chooseTemplateButton.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();

    const wordpressButton = [...document.body.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Wordpress"),
    ) as HTMLButtonElement;
    expect(wordpressButton).toBeDefined();
    wordpressButton.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    const applyButton = [...document.body.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Apply template"),
    ) as HTMLButtonElement;
    expect(applyButton).toBeDefined();
    const backButton = [...document.body.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Back to templates"),
    ) as HTMLButtonElement;
    expect(backButton).toBeDefined();
    backButton.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    expect(document.body.textContent).toContain("Choose a template");
    const wordpressAgainButton = [...document.body.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Wordpress"),
    ) as HTMLButtonElement;
    wordpressAgainButton.click();
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    const reopenedApplyButton = [...document.body.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Apply template"),
    ) as HTMLButtonElement;
    expect(reopenedApplyButton).toBeDefined();
    reopenedApplyButton.click();
    await nextTick();
    expect((host.querySelector("#service-config-compose-yaml") as HTMLTextAreaElement).value).toBe(
      "services:\n  wordpress:\n    image: wordpress:latest\n",
    );
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      kind: "compose",
      compose_yaml: "services:\n  wordpress:\n    image: wordpress:latest\n",
      exposed_service: "wordpress",
      internal_port: 80,
      source_config: { source: "template", template: "wordpress" },
    });
    const savedVariables = (
      onSave.mock.calls[0]?.[0] as
        | {
            variables?: Array<{
              key: string;
              value: string;
              is_secret: boolean;
            }>;
          }
        | undefined
    )?.variables;
    expect(savedVariables?.find((variable) => variable.key === "SITE_NAME")).toEqual({
      key: "SITE_NAME",
      value: "Ignitify",
      is_secret: false,
    });
    const savedPassword = savedVariables?.find((variable) => variable.key === "ADMIN_PASSWORD");
    expect(savedPassword?.is_secret).toBe(true);
    expect(savedPassword?.value.length).toBe(32);
    expect(/^[A-Za-z0-9_-]+$/.test(savedPassword?.value ?? "")).toBe(true);
    app.unmount();
  });

  it("saves a Git Compose source without inline YAML", async () => {
    providerApi.repositories.mockResolvedValue({
      success: true,
      data: [{ name: "stack", path: "acme/stack", default_branch: "main" }],
    });
    providerApi.branches.mockResolvedValue({ success: true, data: [{ name: "main" }] });
    const component = (await import("./ServiceConfigurationPanel.vue")).default;
    const onSave = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      providers: [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      inheritedVariables: [],
      saving: false,
      error: null,
      onSave,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    const textButton = (label: string) =>
      [...host.querySelectorAll("button")].find((button) => button.textContent?.includes(label));
    (textButton("Compose") as HTMLButtonElement).click();
    await nextTick();
    (textButton("Provider repository") as HTMLButtonElement).click();
    await nextTick();
    await selectOption(host, "service-config-compose-provider", "GitHub");
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    await selectOption(host, "service-config-compose-repository", "acme/stack");
    await new Promise((resolve) => setTimeout(resolve, 0));
    await nextTick();
    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      kind: "compose",
      source_config: {
        source: "compose",
        provider_id: "provider-1",
        repository: "acme/stack",
        branch: "main",
        dockerfile_path: "docker-compose.yml",
      },
    });
    const saved = onSave.mock.calls[0]?.[0] as ServiceInput | undefined;
    expect(saved?.compose_yaml).toBeUndefined();
    expect(saved?.exposed_service).toBeUndefined();
    expect(host.querySelector("#service-config-exposed")).toBeNull();
    app.unmount();
  });
});
