// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { shallowRef } from "vue";
import { useSystemMonitoring } from "./useSystemMonitoring";

const api = vi.hoisted(() => ({
  getSystemMetrics: vi.fn(),
}));

vi.mock("@/lib/api/dashboard", () => ({
  apiGetSystemMetrics: api.getSystemMetrics,
}));

afterEach(() => {
  api.getSystemMetrics.mockReset();
});

describe("useSystemMonitoring", () => {
  it("does not request local metrics while a remote destination is active", async () => {
    const enabled = shallowRef(false);
    const monitoring = useSystemMonitoring({ enabled });

    await monitoring.refresh();
    expect(api.getSystemMetrics).not.toHaveBeenCalled();

    api.getSystemMetrics.mockResolvedValueOnce({
      data: {
        block_read_bytes_per_second: 0,
        block_write_bytes_per_second: 0,
        cpu_cores: 2,
        cpu_usage_percentage: 4,
        disk_total_bytes: 2_000,
        disk_used_bytes: 1_000,
        docker_disk_total_bytes: null,
        docker_disk_used_bytes: null,
        memory_total_bytes: 2_000,
        memory_used_bytes: 1_000,
        network_receive_bytes_per_second: 0,
        network_transmit_bytes_per_second: 0,
      },
      success: true,
    });
    enabled.value = true;

    await monitoring.refresh();
    expect(api.getSystemMetrics).toHaveBeenCalledTimes(1);
  });
});
