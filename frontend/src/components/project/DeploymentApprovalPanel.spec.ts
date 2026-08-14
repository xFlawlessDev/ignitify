// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

afterEach(() => {
  document.body.replaceChildren();
});

describe("DeploymentApprovalPanel", () => {
  it("lets an authorized operator approve a pending production deployment", async () => {
    const component = (await import("./DeploymentApprovalPanel.vue")).default;
    const onApprove = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      approval: { status: "pending", requested_at: "2026-08-14T00:00:00Z" },
      canApprove: true,
      submitting: false,
      onApprove,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    const button = [...host.querySelectorAll("button")].find((element) =>
      element.textContent?.includes("Approve production deployment"),
    ) as HTMLButtonElement;
    button.click();

    expect(onApprove).toHaveBeenCalledTimes(1);
    expect(host.textContent).toContain("Approval pending");
    app.unmount();
  });

  it("shows the approval requirement without granting the action to editors", async () => {
    const component = (await import("./DeploymentApprovalPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      approval: { status: "pending", requested_at: "2026-08-14T00:00:00Z" },
      canApprove: false,
      submitting: false,
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("must approve this deployment");
    expect(host.querySelector("button")).toBeNull();
    app.unmount();
  });
});
