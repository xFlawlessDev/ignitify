// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from "vitest";
import { createApp, nextTick } from "vue";
import type { ActivitySummary, DeploymentSummary, ServiceSummary } from "@/lib/types";

const services: ServiceSummary[] = [
  {
    id: "service-web",
    project_id: "project-1",
    environment_id: "environment-1",
    role: "owner",
    name: "web",
    kind: "image",
    image_reference:
      "nginx@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    internal_port: 80,
    healthcheck: null,
    desired_generation: 2,
    desired_state: "running",
    created_at: "2026-08-07T00:00:00Z",
    updated_at: "2026-08-07T00:00:00Z",
    variables: [],
  },
  {
    id: "service-api",
    project_id: "project-1",
    environment_id: "environment-1",
    role: "owner",
    name: "api",
    kind: "image",
    image_reference:
      "nginx@sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    internal_port: 3000,
    healthcheck: null,
    desired_generation: 1,
    desired_state: "stopped",
    created_at: "2026-08-07T00:00:00Z",
    updated_at: "2026-08-07T00:00:00Z",
    variables: [],
  },
];

const deployments: DeploymentSummary[] = [
  {
    id: "deployment-web",
    service_id: "service-web",
    generation: 2,
    status: "healthy",
    failure_reason: null,
    attempt_count: 1,
    retry_after: null,
    cancel_requested_at: null,
    created_at: "2026-08-07T10:00:00Z",
    started_at: "2026-08-07T10:00:01Z",
    finished_at: null,
  },
  {
    id: "deployment-api",
    service_id: "service-api",
    generation: 1,
    status: "failed",
    failure_reason: "Image pull failed",
    attempt_count: 3,
    retry_after: null,
    cancel_requested_at: null,
    created_at: "2026-08-07T09:00:00Z",
    started_at: "2026-08-07T09:00:01Z",
    finished_at: "2026-08-07T09:00:02Z",
  },
];

const activity: ActivitySummary[] = [
  {
    id: "activity-1",
    action: "service.created",
    resource_type: "service",
    resource_id: "service-web",
    created_at: "2026-08-07T10:00:00Z",
  },
];

afterEach(() => {
  document.body.replaceChildren();
});

describe("ProjectOverviewPanel", () => {
  it("summarizes service, deployment, and activity data", async () => {
    const component = (await import("./ProjectOverviewPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      activity,
      activityError: null,
      activityLoading: false,
      deploymentError: null,
      deployments,
      deploymentsLoading: false,
      serviceError: null,
      services,
      servicesLoading: false,
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("Project overview");
    expect(host.textContent).toContain("Running target");
    expect(host.textContent).toContain("web");
    expect(host.textContent).toContain("healthy");
    expect(host.textContent).toContain("service.created");
    app.unmount();
  });

  it("renders empty states after successful empty responses instead of skeletons", async () => {
    const component = (await import("./ProjectOverviewPanel.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      activity: [],
      activityError: null,
      activityLoading: false,
      deploymentError: null,
      deployments: [],
      deploymentsLoading: false,
      serviceError: null,
      services: [],
      servicesLoading: false,
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("No services configured");
    expect(host.textContent).toContain("No deployments yet");
    expect(host.textContent).toContain("No project activity yet.");
    expect(host.querySelector('[role="status"]')).toBeNull();
    app.unmount();
  });
});
