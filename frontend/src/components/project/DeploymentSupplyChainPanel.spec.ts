// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick } from "vue";
import i18n from "@/i18n";

afterEach(() => {
  document.body.replaceChildren();
});

describe("DeploymentSupplyChainPanel", () => {
  it("shows warning-mode remediation without claiming unavailable scan evidence", async () => {
    const component = (await import("./DeploymentSupplyChainPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      report: {
        enforcement: "warning",
        status: "warning",
        provenance: {
          status: "pass",
          summary: "The runtime image is pinned to an immutable digest.",
        },
        sbom: {
          status: "warning",
          summary: "No verified application-image SBOM is attached to this deployment.",
          remediation: "Attach and verify a CycloneDX or SPDX SBOM for the resolved image digest.",
        },
        vulnerabilities: {
          status: "warning",
          summary: "No vulnerability scan result is attached to this deployment.",
          remediation:
            "Record a vulnerability scan for the resolved image digest before enforcing policy.",
        },
        evaluated_at: "2026-08-14T00:00:00Z",
      },
    });
    app.use(i18n);
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Supply-chain policy");
    expect(host.textContent).toContain("Warnings do not block this deployment.");
    expect(host.textContent).toContain("Attach and verify a CycloneDX or SPDX SBOM");
    expect(host.textContent).toContain("Record a vulnerability scan");
    app.unmount();
  });
});
