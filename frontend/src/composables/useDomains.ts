import { shallowRef } from "vue";
import {
  apiCreateDomain,
  apiListDomains,
  apiRemoveDomain,
  apiVerifyDomain,
} from "@/lib/api/domains";
import type { DomainSummary } from "@/lib/types";

export function useDomains() {
  const data = shallowRef<DomainSummary[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load(serviceIds: string[]) {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const results = await Promise.all(serviceIds.map((serviceId) => apiListDomains(serviceId)));
    if (generation !== loadGeneration) return;
    loading.value = false;
    const failed = results.find((result) => !result.success);
    if (failed) {
      error.value = failed.error ?? "Could not load domains";
      return;
    }
    data.value = results.flatMap((result) => result.data);
  }

  function clear() {
    loadGeneration += 1;
    data.value = [];
    loading.value = false;
    error.value = null;
  }

  async function create(serviceId: string, hostname: string): Promise<DomainSummary | null> {
    error.value = null;
    const result = await apiCreateDomain(serviceId, hostname);
    if (!result.success) {
      error.value = result.error ?? "Could not add domain";
      return null;
    }
    data.value = [...data.value, result.data];
    return result.data;
  }

  async function remove(domain: DomainSummary): Promise<boolean> {
    error.value = null;
    const result = await apiRemoveDomain(domain.id, domain.hostname);
    if (!result.success) {
      error.value = result.error ?? "Could not remove domain";
      return false;
    }
    data.value = data.value.filter((item) => item.id !== domain.id);
    return true;
  }

  async function verify(domain: DomainSummary): Promise<boolean> {
    error.value = null;
    const result = await apiVerifyDomain(domain.id);
    if (!result.success) {
      error.value = result.error ?? "Could not start DNS verification";
      return false;
    }
    data.value = data.value.map((item) => (item.id === domain.id ? result.data : item));

    for (let attempt = 0; attempt < 5; attempt += 1) {
      await new Promise((resolve) => window.setTimeout(resolve, 750));
      const refreshed = await apiListDomains(domain.service_id);
      if (!refreshed.success) continue;
      const updated = refreshed.data.find((item) => item.id === domain.id);
      if (!updated) continue;
      data.value = data.value.map((item) => (item.id === domain.id ? updated : item));
      if (updated.dns_status !== "pending") return updated.dns_status === "valid";
    }
    return true;
  }

  return { data, loading, error, load, clear, create, remove, verify };
}
