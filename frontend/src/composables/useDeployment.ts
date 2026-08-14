import { shallowRef } from "vue";
import {
  apiApproveDeployment,
  apiCancelDeployment,
  apiDeployService,
  apiListDeployments,
  apiListProjectDeployments,
  apiRollbackDeployment,
  apiStopService,
} from "@/lib/api/deployments";
import type { DeploymentSummary } from "@/lib/types";

function idempotencyKey() {
  return crypto.randomUUID();
}

export function useDeployment() {
  const data = shallowRef<DeploymentSummary[]>([]);
  const loading = shallowRef(false);
  const submitting = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let pendingLoads = 0;
  let loadGeneration = 0;

  function clear() {
    loadGeneration += 1;
    data.value = [];
    error.value = null;
    pendingLoads = 0;
    loading.value = false;
  }

  function retainServices(serviceIds: string[]) {
    const allowed = new Set(serviceIds);
    data.value = data.value.filter((deployment) => allowed.has(deployment.service_id));
  }

  async function loadProject(projectId: string) {
    const generation = loadGeneration;
    pendingLoads += 1;
    if (pendingLoads === 1) error.value = null;
    loading.value = true;
    try {
      const result = await apiListProjectDeployments(projectId);
      if (generation !== loadGeneration) return;
      if (!result.success) {
        error.value = result.error ?? "Could not load deployments";
        return;
      }
      data.value = result.data;
    } finally {
      if (generation === loadGeneration) {
        pendingLoads -= 1;
        loading.value = pendingLoads > 0;
      }
    }
  }

  async function load(serviceId: string) {
    const generation = loadGeneration;
    pendingLoads += 1;
    if (pendingLoads === 1) error.value = null;
    loading.value = true;
    try {
      const result = await apiListDeployments(serviceId);
      if (generation !== loadGeneration) return;
      if (!result.success) {
        error.value = result.error ?? "Could not load deployments";
        return;
      }
      data.value = [
        ...result.data,
        ...data.value.filter((deployment) => deployment.service_id !== serviceId),
      ];
    } finally {
      if (generation === loadGeneration) {
        pendingLoads -= 1;
        loading.value = pendingLoads > 0;
      }
    }
  }

  async function deploy(serviceId: string): Promise<DeploymentSummary | null> {
    submitting.value = true;
    error.value = null;
    const result = await apiDeployService(serviceId, idempotencyKey());
    submitting.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not submit deployment";
      return null;
    }
    data.value = [result.data, ...data.value];
    return result.data;
  }

  async function stop(serviceId: string): Promise<DeploymentSummary | null> {
    submitting.value = true;
    error.value = null;
    const result = await apiStopService(serviceId);
    submitting.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not submit stop";
      return null;
    }
    data.value = data.value.map((deployment) =>
      deployment.id === result.data.id ? result.data : deployment,
    );
    return result.data;
  }

  async function rollback(deploymentId: string): Promise<DeploymentSummary | null> {
    submitting.value = true;
    error.value = null;
    const result = await apiRollbackDeployment(deploymentId, idempotencyKey());
    submitting.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not submit rollback";
      return null;
    }
    data.value = [result.data, ...data.value];
    return result.data;
  }

  async function cancel(deploymentId: string): Promise<DeploymentSummary | null> {
    submitting.value = true;
    error.value = null;
    const result = await apiCancelDeployment(deploymentId);
    submitting.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not cancel deployment";
      return null;
    }
    data.value = data.value.map((deployment) =>
      deployment.id === result.data.id ? result.data : deployment,
    );
    return result.data;
  }

  async function approve(deploymentId: string): Promise<DeploymentSummary | null> {
    submitting.value = true;
    error.value = null;
    const result = await apiApproveDeployment(deploymentId);
    submitting.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not approve deployment";
      return null;
    }
    data.value = data.value.map((deployment) =>
      deployment.id === result.data.id ? result.data : deployment,
    );
    return result.data;
  }

  return {
    data,
    loading,
    submitting,
    error,
    clear,
    retainServices,
    loadProject,
    load,
    deploy,
    stop,
    rollback,
    cancel,
    approve,
  };
}
