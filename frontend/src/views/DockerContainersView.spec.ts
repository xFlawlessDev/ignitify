// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick, shallowRef } from "vue";
import DockerContainersView from "./DockerContainersView.vue";

const mocks = vi.hoisted(() => ({
  containers: undefined as unknown,
  runtime: undefined as unknown,
}));

vi.mock("@/components/runtime/RuntimeStatusPanel.vue", () => ({
  default: { template: "<aside>Runtime status</aside>" },
}));

vi.mock("@/composables/useRuntimeContainers", () => ({
  useRuntimeContainers: () => mocks.containers,
}));

vi.mock("@/composables/useRuntimeStatus", () => ({
  useRuntimeStatus: () => mocks.runtime,
}));

function monitoringState() {
  return {
    containers: {
      data: shallowRef([
        {
          cpu_limit_nano_cpus: 1_000_000_000,
          cpu_percentage: 1.25,
          health: "healthy",
          id: "f0f0f0f0f0f0f0f0",
          image: "nginx:latest",
          managed: true,
          memory_limit_bytes: 536_870_912,
          memory_usage_bytes: 67_108_864,
          name: "web",
          ports: [
            {
              container_port: 80,
              host_ip: "0.0.0.0",
              host_port: 8080,
              protocol: "tcp",
            },
          ],
          restart_count: 2,
          state: "running",
          status: "Up 2 minutes",
        },
      ]),
      error: shallowRef<string | null>(null),
      load: vi.fn(),
      loading: shallowRef(false),
    },
    runtime: {
      data: shallowRef({
        database: "ready" as const,
        metrics: null,
        runtime: "ready" as const,
        worker: "ready" as const,
      }),
      error: shallowRef<string | null>(null),
      load: vi.fn(),
      loading: shallowRef(false),
    },
  };
}

async function mount() {
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(DockerContainersView);
  app.mount(host);
  await nextTick();
  return { app, host };
}

afterEach(() => {
  document.body.replaceChildren();
});

describe("DockerContainersView", () => {
  it("renders Docker status, state, and port mapping separately", async () => {
    const state = monitoringState();
    mocks.containers = state.containers;
    mocks.runtime = state.runtime;
    const mounted = await mount();

    expect(mounted.host.textContent).toContain("Status");
    expect(mounted.host.textContent).toContain("State");
    expect(mounted.host.textContent).toContain("Ports");
    expect(mounted.host.textContent).toContain("Up 2 minutes");
    expect(mounted.host.textContent).toContain("running");
    expect(mounted.host.textContent).toContain("0.0.0.0:8080 → 80/tcp");

    mounted.app.unmount();
  });

  it("paginates container inventory", async () => {
    const state = monitoringState();
    const first = state.containers.data.value[0]!;
    state.containers.data.value = Array.from({ length: 11 }, (_, index) => ({
      ...first,
      id: `container-${index + 1}`,
      name: `web-${String(index + 1).padStart(2, "0")}`,
    }));
    mocks.containers = state.containers;
    mocks.runtime = state.runtime;
    const mounted = await mount();

    expect(mounted.host.textContent).toContain("web-01");
    expect(mounted.host.textContent).toContain("web-10");
    expect(mounted.host.textContent).not.toContain("web-11");
    expect(mounted.host.textContent).toContain("Showing 1–10 of 11 containers");
    expect(mounted.host.textContent).toContain("Page 1 of 2");

    (mounted.host.querySelector('button[aria-label="Next page"]') as HTMLButtonElement).click();
    await nextTick();

    expect(mounted.host.textContent).not.toContain("web-01");
    expect(mounted.host.textContent).toContain("web-11");
    expect(mounted.host.textContent).toContain("Showing 11–11 of 11 containers");
    expect(mounted.host.textContent).toContain("Page 2 of 2");

    mounted.app.unmount();
  });
});
