// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, nextTick, shallowRef } from "vue";

const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  load: vi.fn(),
  push: vi.fn(),
  state: undefined as unknown,
}));

vi.mock("@/composables/useProjects", () => ({
  useProjects: () => mocks.state,
}));

vi.mock("vue-router", () => ({
  RouterLink: { template: "<a><slot /></a>" },
  useRouter: () => ({ push: mocks.push }),
}));

function project() {
  return {
    id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
    name: "Platform",
    owner_id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
    role: "owner" as const,
    created_at: "2026-07-31T00:00:00Z",
    updated_at: "2026-07-31T00:00:00Z",
    default_environment: {
      id: "0e4c6e6b-6612-4f43-b3e9-a69fdd780cd9",
      name: "production",
      is_default: true,
    },
  };
}

function state(
  options: {
    loading?: boolean;
    error?: string | null;
    data?: ReturnType<typeof project>[];
  } = {},
) {
  return {
    data: shallowRef(options.data ?? []),
    loading: shallowRef(options.loading ?? false),
    error: shallowRef(options.error ?? null),
    load: mocks.load,
    create: mocks.create,
  };
}

async function mount() {
  const component = (await import("./ProjectsView.vue")).default;
  const host = document.createElement("div");
  document.body.append(host);
  const app = createApp(component);
  app.mount(host);
  await nextTick();
  return { app, host };
}

afterEach(() => {
  document.body.replaceChildren();
  mocks.create.mockReset();
  mocks.load.mockReset();
  mocks.push.mockReset();
});

describe("ProjectsView", () => {
  it("renders loading, error, and empty states", async () => {
    const projects = state({ loading: true });
    mocks.state = projects;
    const mounted = await mount();
    expect(mounted.host.textContent).toContain("Loading projects...");

    projects.loading.value = false;
    projects.error.value = "offline";
    await nextTick();
    expect(mounted.host.textContent).toContain("offline");

    projects.error.value = null;
    await nextTick();
    expect(mounted.host.textContent).toContain("No projects yet");
    mounted.app.unmount();
  });

  it("routes to created project", async () => {
    mocks.state = state();
    mocks.create.mockResolvedValue(project());
    const { app, host } = await mount();

    (host.querySelector("button") as HTMLButtonElement).click();
    await nextTick();
    const input = document.querySelector("#project-name") as HTMLInputElement;
    input.value = "Platform";
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    input.form!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await nextTick();
    await nextTick();

    expect(mocks.create.mock.calls).toEqual([[{ name: "Platform" }]]);
    expect(mocks.push.mock.calls).toEqual([
      [{ name: "ProjectDetail", params: { projectId: project().id } }],
    ]);
    app.unmount();
  });
});
