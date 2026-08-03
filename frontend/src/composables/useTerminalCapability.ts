import { shallowRef } from "vue";
import { apiGetTerminalCapability } from "@/lib/api/dashboard";
import type { TerminalCapability } from "@/lib/types";

export function useTerminalCapability() {
  const data = shallowRef<TerminalCapability | null>(null);
  const error = shallowRef<string | null>(null);
  const loading = shallowRef(false);

  async function load(serviceId: string) {
    loading.value = true;
    error.value = null;
    const result = await apiGetTerminalCapability(serviceId);
    if (result.success) data.value = result.data;
    else error.value = result.error ?? "Could not load terminal capability";
    loading.value = false;
  }

  return { data, error, loading, load };
}
