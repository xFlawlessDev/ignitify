// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick } from "vue";

afterEach(() => {
  document.body.replaceChildren();
});

describe("ProjectActivityPanel", () => {
  it("paginates project activity entries", async () => {
    const component = (await import("./ProjectActivityPanel.vue")).default;
    const activity = Array.from({ length: 11 }, (_, index) => ({
      id: `activity-${index + 1}`,
      action: `Activity ${index + 1}`,
      resource_type: "service",
      resource_id: `service-${index + 1}`,
      created_at: `2026-08-${String(index + 1).padStart(2, "0")}T00:00:00Z`,
    }));
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, { activity, error: null, loading: false });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Showing 1–10 of 11 activity entries");
    expect(host.textContent).toContain("Activity 1");
    expect(host.textContent).not.toContain("Activity 11");

    (host.querySelector('button[aria-label="Next activity page"]') as HTMLButtonElement).click();
    await nextTick();

    expect(host.textContent).toContain("Showing 11–11 of 11 activity entries");
    expect(host.textContent).toContain("Activity 11");
    app.unmount();
  });
});
