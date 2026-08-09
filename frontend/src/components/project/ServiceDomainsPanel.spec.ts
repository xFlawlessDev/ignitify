// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";

const service = {
  id: "service-1",
  project_id: "project-1",
  environment_id: "environment-1",
  role: "owner" as const,
  name: "web",
  kind: "image" as const,
  internal_port: 3000,
  healthcheck: null,
  desired_generation: 1,
  desired_state: "running" as const,
  created_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  variables: [],
};

afterEach(() => {
  document.body.replaceChildren();
});

describe("ServiceDomainsPanel", () => {
  it("creates a route for a service port and exposes DNS and public link actions", async () => {
    const component = (await import("./ServiceDomainsPanel.vue")).default;
    const onCreate = vi.fn();
    const onVerify = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      canManage: true,
      domains: [
        {
          id: "domain-1",
          service_id: service.id,
          hostname: "app.example.com",
          status: "active" as const,
          last_error: null,
          dns_record_type: "a" as const,
          dns_record_target: "203.0.113.10",
          dns_status: "not_checked" as const,
          dns_error: null,
          dns_checked_at: null,
          created_at: "2026-08-01T00:00:00Z",
          updated_at: "2026-08-01T00:00:00Z",
        },
      ],
      error: null,
      loading: false,
      services: [service],
      fixedServiceId: service.id,
      onCreate,
      onVerify,
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Custom domain");
    expect(host.textContent).toContain("DNS not verified");
    expect(host.textContent).toContain("A 203.0.113.10");
    expect(host.textContent).toContain("Verify DNS");
    expect(host.textContent).toContain("Open link");
    expect(host.textContent).toContain("This route targets the current service.");
    expect(host.querySelector("#domain-service")).toBeNull();

    const domainInput = host.querySelector("#project-domain") as HTMLInputElement;
    domainInput.value = "api.example.com";
    domainInput.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();

    (host.querySelector("form") as HTMLFormElement).dispatchEvent(
      new Event("submit", { bubbles: true, cancelable: true }),
    );
    await nextTick();

    expect(onCreate.mock.calls[0]).toEqual(["service-1", "api.example.com"]);
    (host.querySelector('button[title="Verify DNS"]') as HTMLButtonElement).click();
    await nextTick();
    const verified = onVerify.mock.calls[0]?.[0] as { id: string };
    expect(verified.id).toBe("domain-1");
    expect((host.querySelector('a[title="Open link"]') as HTMLAnchorElement).href).toBe(
      "https://app.example.com/",
    );
    app.unmount();
  });
});
