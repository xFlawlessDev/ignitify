// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";

async function mount(onSave = () => {}) {
  const component = (await import("./ServiceDialog.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component, {
    error: null,
    saving: false,
    service: null,
    open: true,
    "onUpdate:open": () => {},
    onSave,
  });
  app.mount(host);
  await nextTick();
  return { app, host };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("ServiceDialog", () => {
  it("blocks invalid internal ports before emitting", async () => {
    const onSave = vi.fn();
    const { app } = await mount(onSave);
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
    const { app } = await mount();
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
});
