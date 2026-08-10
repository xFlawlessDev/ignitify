// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick } from "vue";

afterEach(() => {
  document.body.replaceChildren();
});

describe("DeploymentLogsPanel", () => {
  it("renders ANSI deployment output without exposing escape sequences", async () => {
    const component = (await import("./DeploymentLogsPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      connected: true,
      logs: [
        {
          sequence: 1,
          deployment_id: "deployment-1",
          stream: "stderr",
          line: "\u001b[31mBuild failed\u001b[0m",
          created_at: "2026-08-10T00:00:00Z",
        },
      ],
      streamError: null,
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Build failed");
    expect(host.innerHTML).not.toContain("\u001b[31m");
    expect(host.querySelector("span[style*='color']")).not.toBeNull();
    app.unmount();
  });
});
