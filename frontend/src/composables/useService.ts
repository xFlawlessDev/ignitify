import { shallowRef } from "vue";
import {
  apiCreateService,
  apiDeleteService,
  apiGetService,
  apiListServices,
  apiRotateAutoDeploySecret,
  apiUpdateService,
} from "@/lib/api/services";
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
    try {
      const result = await apiListServices(projectId);
      if (generation !== loadGeneration) return;
      if (!result.success) {
        data.value = [];
        error.value = result.error ?? "Could not load services";
        return;
      }
      data.value = result.data;
    } catch (cause) {
      if (generation !== loadGeneration) return;
      data.value = [];
      error.value = cause instanceof Error ? cause.message : "Could not load services";
    } finally {
      if (generation === loadGeneration) loading.value = false;
    }
  }

  async function create(projectId: string, input: ServiceInput): Promise<ServiceSummary | null> {
    error.value = null;
    const result = await apiCreateService(projectId, input);
    if (!result.success) {
      error.value = result.error ?? "Could not create service";
      return null;
    }
    // A create result is newer than a pending list response, so ignore that stale response.
    loadGeneration += 1;
    loading.value = false;
    data.value = [result.data, ...data.value];
    return result.data;
  }

  async function get(serviceId: string): Promise<ServiceSummary | null> {
    error.value = null;
    try {
      const result = await apiGetService(serviceId);
      if (!result.success) {
        error.value = result.error ?? "Could not load service";
        return null;
      }
      const service = result.data;
      data.value = data.value.some((item) => item.id === service.id)
        ? data.value.map((item) => (item.id === service.id ? service : item))
        : [service, ...data.value];
      return service;
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Could not load service";
      return null;
    }
  }

  async function update(serviceId: string, input: ServiceInput): Promise<ServiceSummary | null> {
    error.value = null;
    const result = await apiUpdateService(serviceId, input);
    if (!result.success) {
      error.value = result.error ?? "Could not update service";
      return null;
    }
    data.value = data.value.map((item) => (item.id === serviceId ? result.data : item));
    return result.data;
  }

  async function remove(serviceId: string, confirmName: string): Promise<boolean> {
    error.value = null;
    const result = await apiDeleteService(serviceId, confirmName);
    if (!result.success) {
      error.value = result.error ?? "Could not delete service";
      return false;
    }
    loadGeneration += 1;
    loading.value = false;
    data.value = data.value.filter((service) => service.id !== serviceId);
    return true;
  }

  async function rotateAutoDeploySecret(serviceId: string): Promise<string | null> {
    error.value = null;
    const result = await apiRotateAutoDeploySecret(serviceId);
    if (!result.success) {
      error.value = result.error ?? "Could not rotate auto-deploy secret";
      return null;
    }
    const secret = result.data.auto_deploy_webhook_secret;
    data.value = data.value.map((item) =>
      item.id === serviceId ? { ...item, auto_deploy_webhook_secret: secret } : item,
    );
    return secret;
  }

  return { data, loading, error, load, get, create, update, remove, rotateAutoDeploySecret };
}
