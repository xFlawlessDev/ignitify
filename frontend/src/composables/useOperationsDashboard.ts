import { computed, shallowRef } from "vue";
import { apiGetDashboard, apiGetRuntimeStatus } from "@/lib/api/dashboard";
import type {
  DashboardProjectSummary,
  DashboardServiceSummary,
  DashboardSummary,
  DeploymentSummary,
  RuntimeStatus,
} from "@/lib/types";

export interface DashboardDeployment {
  deployment: DeploymentSummary;
  project: DashboardProjectSummary;
  service: DashboardServiceSummary | null;
}

const emptyData = (): DashboardSummary => ({ deployments: [], projects: [], services: [] });

function compareCreatedAt(left: DeploymentSummary, right: DeploymentSummary) {
  return right.created_at.localeCompare(left.created_at);
}

export function useOperationsDashboard() {
  const data = shallowRef<DashboardSummary>(emptyData());
  const runtime = shallowRef<RuntimeStatus | null>(null);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  const metrics = computed(() => {
    const latestByService = new Map<string, DeploymentSummary>();
    for (const deployment of data.value.deployments) {
      if (!latestByService.has(deployment.service_id)) {
        latestByService.set(deployment.service_id, deployment);
      }
    }
    const latest = [...latestByService.values()];

    return {
      active: latest.filter((deployment) =>
        ["queued", "preparing", "running", "stopping"].includes(deployment.status),
      ).length,
      failed: latest.filter((deployment) => deployment.status === "failed").length,
      healthy: latest.filter((deployment) => deployment.status === "healthy").length,
      projects: data.value.projects.length,
      services: data.value.services.length,
    };
  });

  const recentDeployments = computed<DashboardDeployment[]>(() => {
    const projects = new Map(data.value.projects.map((project) => [project.id, project]));
    const services = new Map(data.value.services.map((service) => [service.id, service]));

    return [...data.value.deployments]
      .sort(compareCreatedAt)
      .flatMap((deployment) => {
        const service = services.get(deployment.service_id) ?? null;
        const project = service ? projects.get(service.project_id) : undefined;
        return project ? [{ deployment, project, service }] : [];
      })
      .slice(0, 5);
  });

  async function load() {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;

    try {
      const [dashboardResult, runtimeResult] = await Promise.all([
        apiGetDashboard(),
        apiGetRuntimeStatus(),
      ]);
      if (generation !== loadGeneration) return;

      if (!dashboardResult.success) {
        data.value = emptyData();
        runtime.value = null;
        error.value = dashboardResult.error ?? "Could not load workspace operations";
        return;
      }

      data.value = dashboardResult.data;
      if (runtimeResult.success) {
        runtime.value = runtimeResult.data;
      } else {
        runtime.value = null;
        error.value = runtimeResult.error ?? "Could not load runtime status";
      }
    } catch {
      if (generation === loadGeneration) {
        data.value = emptyData();
        runtime.value = null;
        error.value = "Could not load workspace operations";
      }
    } finally {
      if (generation === loadGeneration) loading.value = false;
    }
  }

  return { data, error, loading, load, metrics, recentDeployments, runtime };
}
