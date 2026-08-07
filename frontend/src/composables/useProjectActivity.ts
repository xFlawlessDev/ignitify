import { shallowRef } from "vue";
import { apiListProjectActivity } from "@/lib/api/activity";
import type { ActivitySummary } from "@/lib/types";

export function useProjectActivity() {
  const data = shallowRef<ActivitySummary[]>([]);
  const error = shallowRef<string | null>(null);
  const loading = shallowRef(false);

  async function load(projectId: string) {
    loading.value = true;
    error.value = null;
    try {
      const result = await apiListProjectActivity(projectId);
      if (result.success) data.value = result.data;
      else error.value = result.error ?? "Could not load project activity";
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Could not load project activity";
    } finally {
      loading.value = false;
    }
  }

  return { data, error, loading, load };
}
