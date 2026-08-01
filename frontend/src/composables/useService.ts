import { shallowRef } from "vue";
import { apiCreateService, apiListServices, apiUpdateService } from "@/lib/api/services";
import type { ServiceInput, ServiceSummary } from "@/lib/types";

export function useService() {
  const data = shallowRef<ServiceSummary[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load(projectId: string) {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const result = await apiListServices(projectId);
    if (generation !== loadGeneration) return;
    loading.value = false;
    if (!result.success) {
      data.value = [];
      error.value = result.error ?? "Could not load services";
      return;
    }
    data.value = result.data;
  }

  async function create(projectId: string, input: ServiceInput): Promise<ServiceSummary | null> {
    error.value = null;
    const result = await apiCreateService(projectId, input);
    if (!result.success) {
      error.value = result.error ?? "Could not create service";
      return null;
    }
    data.value = [result.data, ...data.value];
    return result.data;
  }

  async function update(serviceId: string, input: ServiceInput): Promise<ServiceSummary | null> {
    error.value = null;
    const result = await apiUpdateService(serviceId, input);
    if (!result.success) {
      error.value = result.error ?? "Could not update service";
      return null;
    }
    data.value = data.value.map((service) => (service.id === serviceId ? result.data : service));
    return result.data;
  }

  return { data, loading, error, load, create, update };
}
