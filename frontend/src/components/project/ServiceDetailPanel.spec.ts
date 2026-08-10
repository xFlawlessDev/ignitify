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
  image_reference: "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  internal_port: 8080,
  healthcheck: null,
  desired_generation: 2,
  desired_state: "stopped" as const,
  created_at: "2026-08-01T00:00:00Z",
  updated_at: "2026-08-01T00:00:00Z",
  variables: [],
};

afterEach(() => {
  document.body.replaceChildren();
});

describe("ServiceDetailPanel", () => {
  it("exposes deployment actions and the log tabs for a selected service", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const onDeploy = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      deployments: [],
      logs: [],
      connected: false,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy,
      onStop: vi.fn(),
      onRollback: vi.fn(),
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const textButton = (label: string) =>
      [...host.querySelectorAll("button")].find((button) => button.textContent?.includes(label));
    (textButton("Deploy") as HTMLButtonElement).click();
    (textButton("Deployments") as HTMLButtonElement).click();
    await nextTick();

    expect(onDeploy.mock.calls).toEqual([["service-1"]]);
    expect(host.textContent).toContain("Deploy this service to create its first revision.");
    app.unmount();
  });

  it("wires stop and rollback to their dedicated service actions", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const onStop = vi.fn();
    const onRollback = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: { ...service, desired_state: "running" as const },
      deployments: [
        {
          id: "deployment-failed",
          service_id: "service-1",
          generation: 3,
          status: "running" as const,
          failure_reason: null,
          created_at: "2026-08-01T00:00:00Z",
          started_at: "2026-08-01T00:00:01Z",
          finished_at: "2026-08-01T00:00:02Z",
        },
        {
          id: "deployment-healthy",
          service_id: "service-1",
          generation: 2,
          status: "healthy" as const,
          failure_reason: null,
          created_at: "2026-07-31T00:00:00Z",
          started_at: "2026-07-31T00:00:01Z",
          finished_at: null,
        },
      ],
      logs: [],
      connected: true,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy: vi.fn(),
      onStop,
      onRollback,
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const textButton = (label: string) =>
      [...host.querySelectorAll("button")].find((button) => button.textContent?.includes(label));
    (textButton("Stop") as HTMLButtonElement).click();
    (textButton("Deployments") as HTMLButtonElement).click();
    await nextTick();
    (textButton("Rollback") as HTMLButtonElement).click();

    expect(onStop.mock.calls).toEqual([["service-1"]]);
    expect(onRollback.mock.calls).toEqual([["deployment-healthy"]]);
    app.unmount();
  });

  it("shows stop for an active deployment even when desired state is stopped", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const onStop = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      deployments: [
        {
          id: "deployment-healthy",
          service_id: "service-1",
          generation: 1,
          status: "healthy" as const,
          failure_reason: null,
          created_at: "2026-08-01T00:00:00Z",
          started_at: "2026-08-01T00:00:01Z",
          finished_at: null,
        },
      ],
      logs: [],
      connected: true,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy: vi.fn(),
      onStop,
      onRollback: vi.fn(),
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const stopButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Stop"),
    ) as HTMLButtonElement;
    stopButton.click();

    expect(onStop.mock.calls).toEqual([["service-1"]]);
    app.unmount();
  });

  it("labels application redeploys as rebuilds", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: {
        ...service,
        source_config: {
          source: "application",
          repository: "acme/site",
          builder: "dockerfile",
        },
      },
      deployments: [
        {
          id: "deployment-healthy",
          service_id: "service-1",
          generation: 1,
          status: "healthy" as const,
          failure_reason: null,
          created_at: "2026-08-01T00:00:00Z",
          started_at: "2026-08-01T00:00:01Z",
          finished_at: null,
        },
      ],
      logs: [],
      connected: true,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy: vi.fn(),
      onStop: vi.fn(),
      onRollback: vi.fn(),
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Rebuild");
    app.unmount();
  });

  it("blocks deployment until a starter service is configured", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service: {
        ...service,
        source_config: { source: "template", template: "starter", setup_required: true },
      },
      deployments: [],
      logs: [],
      connected: false,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy: vi.fn(),
      onStop: vi.fn(),
      onRollback: vi.fn(),
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const deployButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Deploy"),
    ) as HTMLButtonElement;
    expect(deployButton.disabled).toBe(true);
    expect(host.textContent).toContain("Setup required");
    app.unmount();
  });

  it("paginates deployment history", async () => {
    const component = (await import("./ServiceDetailPanel.vue")).default;
    const deployments = Array.from({ length: 7 }, (_, index) => ({
      id: `deployment-${7 - index}`,
      service_id: "service-1",
      generation: 7 - index,
      status: "stopped" as const,
      failure_reason: null,
      created_at: `2026-08-${String(7 - index).padStart(2, "0")}T00:00:00Z`,
      started_at: null,
      finished_at: "2026-08-01T00:01:00Z",
    }));
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      service,
      deployments,
      logs: [],
      connected: false,
      streamError: null,
      submitting: false,
      canManage: true,
      selectedDeploymentId: null,
      onDeploy: vi.fn(),
      onStop: vi.fn(),
      onRollback: vi.fn(),
      onEdit: vi.fn(),
      onSelectDeployment: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const deploymentsTab = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Deployments"),
    ) as HTMLButtonElement;
    deploymentsTab.click();
    await nextTick();

    expect(host.textContent).toContain("Showing 1–6 of 7 deployments");
    expect(host.textContent).toContain("Generation 7");
    expect(host.textContent).not.toContain("Generation 1");

    (host.querySelector('button[aria-label="Next deployment page"]') as HTMLButtonElement).click();
    await nextTick();

    expect(host.textContent).toContain("Showing 7–7 of 7 deployments");
    expect(host.textContent).toContain("Generation 1");
    app.unmount();
  });
});
