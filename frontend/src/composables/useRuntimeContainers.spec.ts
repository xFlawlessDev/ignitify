// @vitest-environment happy-dom
import { shallowRef } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useRuntimeContainers } from "./useRuntimeContainers";

const api = vi.hoisted(() => ({
  getRuntimeContainers: vi.fn(),
}));

vi.mock("@/lib/api/dashboard", () => ({
  apiGetRuntimeContainers: api.getRuntimeContainers,
}));

afterEach(() => {
  api.getRuntimeContainers.mockReset();
});

describe("useRuntimeContainers", () => {
  it("keeps an unavailable runtime distinct from an empty Docker inventory", async () => {
    api.getRuntimeContainers.mockResolvedValueOnce({
      data: { containers: null },
      success: true,
    });
    const inventory = useRuntimeContainers();

    await inventory.load();

    expect(inventory.data.value).toBeNull();
    expect(inventory.error.value).toBeNull();

    api.getRuntimeContainers.mockResolvedValueOnce({
      data: { containers: [] },
      success: true,
    });
    await inventory.load();

    expect(inventory.data.value).toEqual([]);
  });

  it("reports failures without replacing the last inventory", async () => {
    api.getRuntimeContainers
      .mockResolvedValueOnce({
        data: {
          containers: [
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
          ],
        },
        success: true,
      })
      .mockResolvedValueOnce({
        data: { containers: null },
        error: "Docker offline",
        success: false,
      });
    const inventory = useRuntimeContainers();

    await inventory.load();
    await inventory.load();

    expect(inventory.data.value).toHaveLength(1);
    expect(inventory.error.value).toBe("Docker offline");
  });

  it("loads inventory from the selected remote destination", async () => {
    api.getRuntimeContainers.mockResolvedValueOnce({
      data: { containers: [] },
      success: true,
    });
    const destination = shallowRef("remote-vps");
    const inventory = useRuntimeContainers(destination);

    await inventory.load();

    expect(api.getRuntimeContainers.mock.calls[0]?.[0]).toBe("remote-vps");
  });
});
