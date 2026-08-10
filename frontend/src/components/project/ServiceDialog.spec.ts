// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";
import type { ServiceSummary } from "@/lib/types";

async function mount(
  onSave = () => {},
  inheritedVariables: { key: string; value: string; is_secret: boolean }[] = [],
  providers: { id: string; name: string; kind: string; token_configured: boolean }[] = [],
  service: ServiceSummary | null = null,
) {
  const component = (await import("./ServiceDialog.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component, {
    error: null,
    saving: false,
    service,
    inheritedVariables,
    providers,
    open: true,
    "onUpdate:open": () => {},
    onSave,
  });
  app.use(i18n);
  app.mount(host);
  await nextTick();
  return { app, host };
}

async function selectOption(triggerId: string, label: string) {
  const trigger = document.querySelector(`#${triggerId}`) as HTMLButtonElement;
  trigger.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
  await nextTick();
  const option = [...document.body.querySelectorAll('[data-slot="select-item"]')].find((item) =>
    item.textContent?.includes(label),
  ) as HTMLElement | undefined;
  expect(option).toBeDefined();
  option?.dispatchEvent(new MouseEvent("pointerup", { bubbles: true, button: 0 }));
  await nextTick();
}

const editableService = {
  id: "service-1",
  project_id: "project-1",
  environment_id: "environment-1",
  role: "owner" as const,
  name: "web",
  kind: "image" as const,
  image_reference: "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  internal_port: 8080,
  healthcheck: null,
  desired_generation: 1,
  desired_state: "stopped" as const,
  created_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  variables: [],
};

afterEach(() => {
  document.body.replaceChildren();
});

describe("ServiceDialog", () => {
  it("blocks invalid internal ports before emitting", async () => {
    const onSave = vi.fn();
    const { app } = await mount(onSave, [], [], editableService);
    const image = document.querySelector("#service-image") as HTMLInputElement;
    image.value = "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    image.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    const port = document.querySelector("#service-port") as HTMLInputElement;
    port.value = "65536";
    port.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    (document.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(document.body.textContent).toContain("Internal port must be between 1 and 65535.");
    expect(onSave).not.toHaveBeenCalled();
    app.unmount();
  });

  it("blocks tag-only images and masks secret inputs", async () => {
    const { app } = await mount(() => {}, [], [], editableService);
    const image = document.querySelector("#service-image") as HTMLInputElement;
    image.value = "nginx:latest";
    image.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    (document.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(document.body.textContent).toContain(
      "Image reference must include an exact sha256 digest.",
    );
    const addButton = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Add variable"),
    ) as HTMLButtonElement;
    addButton.click();
    await nextTick();
    expect((document.querySelector('input[type="password"]') as HTMLInputElement).type).toBe(
      "password",
    );
    app.unmount();
  });

  it("keeps project defaults out of the service override payload", async () => {
    const onSave = vi.fn();
    const { app } = await mount(
      onSave,
      [{ key: "APP_ENV", value: "production", is_secret: false }],
      [],
      editableService,
    );
    const image = document.querySelector("#service-image") as HTMLInputElement;
    image.value = "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    image.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    (document.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls.length).toBe(1);
    const payload = onSave.mock.calls[0]?.[0] as {
      variables: { key: string; value: string; is_secret: boolean }[];
    };
    expect(payload.variables).toEqual([]);
    app.unmount();
  });

  it("emits an application builder source with its provider repository", async () => {
    const onSave = vi.fn();
    const { app } = await mount(
      onSave,
      [],
      [{ id: "provider-1", name: "GitHub", kind: "github", token_configured: true }],
      editableService,
    );
    const applicationButton = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Application"),
    ) as HTMLButtonElement;
    applicationButton.click();
    await nextTick();
    await selectOption("service-provider", "GitHub (github)");
    const repository = document.querySelector("#service-repository") as HTMLInputElement;
    repository.value = "acme/site";
    repository.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    (document.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    const payload = onSave.mock.calls[0]?.[0] as { source_config: Record<string, unknown> };
    expect(payload.source_config).toMatchObject({
      source: "application",
      provider_id: "provider-1",
      repository: "acme/site",
      builder: "static",
    });
    app.unmount();
  });

  it("shows Compose policy guidance before a Compose service is saved", async () => {
    const { app } = await mount(() => {}, [], [], editableService);
    const composeButton = [...document.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Compose"),
    ) as HTMLButtonElement;
    composeButton.click();
    await nextTick();

    expect(document.body.textContent).toContain("Compose requirements");
    expect(document.body.textContent).toContain("Application");
    app.unmount();
  });

  it("creates a starter service from its name before configuration", async () => {
    const onSave = vi.fn();
    const { app } = await mount(onSave);
    const name = document.querySelector("#service-name") as HTMLInputElement;
    name.value = "web";
    name.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    (document.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onSave.mock.calls[0]?.[0]).toMatchObject({
      name: "web",
      kind: "image",
      source_config: { source: "template", template: "starter", setup_required: true },
    });
    expect(document.querySelector("#service-image")).toBeNull();
    app.unmount();
  });
});
