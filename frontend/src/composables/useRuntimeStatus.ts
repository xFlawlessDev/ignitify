import { shallowRef } from "vue";
import { apiGetRuntimeStatus } from "@/lib/api/dashboard";
import type { RuntimeStatus } from "@/lib/types";

export function useRuntimeStatus() {
  const data = shallowRef<RuntimeStatus | null>(null);
  const error = shallowRef<string | null>(null);
  const loading = shallowRef(false);

  async function load() {
    loading.value = true;
    error.value = null;
    const result = await apiGetRuntimeStatus();
    if (result.success) data.value = result.data;
    else error.value = result.error ?? "Could not load runtime status";
    loading.value = false;
  }

  return { data, error, loading, load };
}
