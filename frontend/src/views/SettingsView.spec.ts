// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";

async function mountSettings() {
  const component = (await import("./SettingsView.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component);
  app.mount(host);
  await nextTick();
  return { app, host };
}

afterEach(() => {
  document.body.replaceChildren();
  window.localStorage.clear();
  vi.resetModules();
});

describe("SettingsView", () => {
  it("persists a valid server configuration draft", async () => {
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
    await nextTick();

    expect(
      JSON.parse(window.localStorage.getItem("ignitify.server-settings") ?? "{}"),
    ).toMatchObject({
      serverDomain: "control.example.com",
      httpsEnabled: false,
      automaticallyProvisionSsl: false,
      certificateProvider: "none",
      customCertificates: [],
      concurrentBuilds: 2,
    });
    expect(host.textContent).toContain("Saved locally");
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

  it("adds a custom certificate pair for domain selection", async () => {
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
    await nextTick();

    expect(host.textContent).toContain("Production wildcard");
    expect(host.textContent).toContain("production.crt");
    expect(host.textContent).toContain("production.key");
    app.unmount();
  });
});
