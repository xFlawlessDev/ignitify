// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

const mocks = vi.hoisted(() => ({
  getPolicy: vi.fn(),
  updatePolicy: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  apiGetSupplyChainPolicy: mocks.getPolicy,
  apiUpdateSupplyChainPolicy: mocks.updatePolicy,
}));

const mountedApps: Array<{ unmount: () => void }> = [];

async function settle() {
  for (let index = 0; index < 3; index += 1) {
    await Promise.resolve();
    await nextTick();
  }
}

afterEach(() => {
  for (const app of mountedApps) app.unmount();
  mountedApps.length = 0;
  document.body.replaceChildren();
  mocks.getPolicy.mockReset();
  mocks.updatePolicy.mockReset();
});

describe("SupplyChainPolicySettings", () => {
  beforeEach(() => {
    mocks.getPolicy.mockResolvedValue({
      success: true,
      data: {
        enforcement: "warning",
        updated_at: "2026-08-14T00:00:00Z",
      },
    });
  });

  it("loads the operator policy and explains the compatible default", async () => {
    const component = (await import("./SupplyChainPolicySettings.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component);
    app.use(i18n);
    app.mount(host);
    mountedApps.push(app);
    await settle();

    expect(mocks.getPolicy).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain("Deployment provenance");
    expect(host.textContent).toContain("Warning only");
    expect(host.textContent).toContain("runtime execution continues");
    expect(host.textContent).toContain("Updated 2026-08-14T00:00:00Z");
  });
});
