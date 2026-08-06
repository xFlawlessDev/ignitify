import { shallowRef } from "vue";
import { apiGetRuntimeContainers } from "@/lib/api/dashboard";
import type { RuntimeContainer } from "@/lib/types";

export function useRuntimeContainers() {
  const data = shallowRef<RuntimeContainer[] | null>(null);
  const error = shallowRef<string | null>(null);
  const loading = shallowRef(false);

  async function load() {
    loading.value = true;
    error.value = null;
    const result = await apiGetRuntimeContainers();
    if (result.success) data.value = result.data.containers;
    else error.value = result.error ?? "Could not load Docker container inventory";
    loading.value = false;
  }

  return { data, error, loading, load };
}
