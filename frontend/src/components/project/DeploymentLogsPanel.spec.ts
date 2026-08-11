// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

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
    app.use(i18n).mount(host);
    await nextTick();

    expect(host.textContent).toContain("Build failed");
    expect(host.innerHTML).not.toContain("\u001b[31m");
    expect(host.querySelector("span[style*='color']")).not.toBeNull();
    expect(host.querySelector('button[aria-label="Copy deployment logs"]')).not.toBeNull();
    app.unmount();
  });

  it("filters deployment output by a case-insensitive search query", async () => {
    const component = (await import("./DeploymentLogsPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      connected: false,
      logs: [
        {
          sequence: 1,
          deployment_id: "deployment-1",
          stream: "stdout",
          line: "Listening on port 8080",
          created_at: "2026-08-10T00:00:00Z",
        },
        {
          sequence: 2,
          deployment_id: "deployment-1",
          stream: "stderr",
          line: "Connection timed out",
          created_at: "2026-08-10T00:00:01Z",
        },
      ],
      streamError: null,
    });
    app.use(i18n).mount(host);
    await nextTick();

    const search = host.querySelector('input[type="search"]') as HTMLInputElement;
    search.value = "TIMED OUT";
    search.dispatchEvent(new Event("input"));
    await nextTick();

    expect(host.textContent).toContain("Connection timed out");
    expect(host.textContent).not.toContain("Listening on port 8080");
    expect(host.textContent).toContain("1 of 2 lines");
    app.unmount();
  });
});
