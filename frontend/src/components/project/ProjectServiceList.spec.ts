// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";

function service(index = 0) {
  return {
    id: `0e4c6e6b-6612-4f43-b3e9-a69fdd780cd${index}`,
    project_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
    environment_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
    role: "owner" as const,
    name: index === 0 ? "Web" : `Service ${index + 1}`,
    kind: "image" as const,
    image_reference: "nginx:latest",
    internal_port: 8080,
    healthcheck: null,
    desired_generation: 1,
    desired_state: "running" as const,
    created_at: "2026-07-31T00:00:00Z",
    updated_at: "2026-07-31T00:00:00Z",
    variables: [],
  };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("ProjectServiceList", () => {
  it("keeps Add service available while the list is loading", async () => {
    const component = (await import("./ProjectServiceList.vue")).default;
    const onCreate = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      canManage: true,
      loading: true,
      services: [],
      onCreate,
      onSelect: vi.fn(),
      onEdit: vi.fn(),
      onRetry: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    const addButton = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("Add service"),
    );
    (addButton as HTMLButtonElement).click();

    expect(onCreate).toHaveBeenCalledTimes(1);
    expect(host.querySelector('[role="status"]')).not.toBeNull();
    app.unmount();
  });

  it("renders the empty state after a successful empty response", async () => {
    const component = (await import("./ProjectServiceList.vue")).default;
    const host = document.createElement("div");
    document.body.append(host);
    const app = createApp(component, {
      canManage: true,
      loading: false,
      services: [],
      onCreate: vi.fn(),
      onSelect: vi.fn(),
      onEdit: vi.fn(),
      onRetry: vi.fn(),
    });
    app.mount(host);
    await nextTick();

    expect(host.textContent).toContain("No services configured");
    expect(host.querySelector('[aria-label="Service catalog view"]')).not.toBeNull();
    expect(
      host.querySelector('[aria-label="Service catalog view"]')?.getAttribute("aria-pressed"),
    ).toBe("true");
    expect(host.querySelector('[role="status"]')).toBeNull();
    app.unmount();
  });

  it("renders catalog cards and emits the requested view change", async () => {
    const component = (await import("./ProjectServiceList.vue")).default;
    const onUpdateView = vi.fn();
    const host = document.createElement("div");
    document.body.append(host);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", name: "Home", component: { template: "<div />" } },
        {
          path: "/projects/:projectId/services/:serviceId",
          name: "ServiceDetail",
          component: { template: "<div />" },
        },
      ],
    });
    await router.push("/");
    await router.isReady();
    const app = createApp(component, {
      canManage: true,
      services: [service()],
      view: "catalog",
      onCreate: vi.fn(),
      onEdit: vi.fn(),
      onRetry: vi.fn(),
      onSelect: vi.fn(),
      onUpdateView,
    });
    app.use(router);
    app.mount(host);
    await nextTick();

    expect(host.querySelectorAll("a")).toHaveLength(1);
    expect(host.textContent).toContain("Web");
    expect(host.textContent).toContain("Running");

    (host.querySelector('[aria-label="Service list view"]') as HTMLButtonElement).click();
    expect(onUpdateView).toHaveBeenCalled();
    expect(onUpdateView.mock.calls[0]?.[0]).toBe("list");
    app.unmount();
  });
});
