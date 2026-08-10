import { shallowRef } from "vue";
import { apiGetRuntimeContainers } from "@/lib/api/dashboard";
import { apiGetRuntimeContainerLogs } from "@/lib/api/runtime-containers";
import type { DeploymentSummary, RuntimeContainer, ServiceSummary } from "@/lib/types";

function containerPrefix(service: ServiceSummary, deployment: DeploymentSummary): string {
  if (service.deployment_destination_id) {
    return `ignitify-remote-${service.deployment_destination_id}-service-${service.id}-g${deployment.generation}`;
  }
  if (service.kind === "image") {
    return `ignitify-svc-${service.id}-g${deployment.generation}`;
  }
  return `ignitify-${service.id}-g${deployment.generation}`;
}

function findServiceContainer(
  containers: RuntimeContainer[],
  service: ServiceSummary,
  deployment: DeploymentSummary,
): RuntimeContainer | null {
  const prefix = containerPrefix(service, deployment);
  return (
    containers
      .filter((container) => container.managed && container.name.startsWith(prefix))
      .sort(
        (left, right) => Number(right.state === "running") - Number(left.state === "running"),
      )[0] ?? null
  );
}

export function useServiceRuntimeLogs() {
  const output = shallowRef<string | null>(null);
  const container = shallowRef<RuntimeContainer | null>(null);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  const emptyState = shallowRef<"select_deployment" | "no_container" | "no_output">(
    "select_deployment",
  );
  let requestGeneration = 0;

  function clear() {
    requestGeneration += 1;
    output.value = null;
    container.value = null;
    loading.value = false;
    error.value = null;
    emptyState.value = "select_deployment";
  }

  async function load(service: ServiceSummary, deployment: DeploymentSummary | null) {
    const generation = ++requestGeneration;
    output.value = null;
    container.value = null;
    error.value = null;
    loading.value = true;

    if (!deployment) {
      emptyState.value = "select_deployment";
      loading.value = false;
      return;
    }

    const destination = service.deployment_destination_id ?? undefined;
    const inventory = await apiGetRuntimeContainers(destination);
    if (generation !== requestGeneration) return;
    if (!inventory.success) {
      error.value = inventory.error ?? "Could not load the runtime container inventory.";
      loading.value = false;
      return;
    }

    const matchedContainer = findServiceContainer(
      inventory.data.containers ?? [],
      service,
      deployment,
    );
    if (!matchedContainer) {
      emptyState.value = "no_container";
      loading.value = false;
      return;
    }

    container.value = matchedContainer;
    const logs = await apiGetRuntimeContainerLogs(matchedContainer.id, destination);
    if (generation !== requestGeneration) return;
    if (!logs.success) {
      error.value = logs.error ?? "Could not load service logs.";
      loading.value = false;
      return;
    }

    if (logs.data.logs) output.value = logs.data.logs;
    else emptyState.value = "no_output";
    loading.value = false;
  }

  return { output, container, loading, error, emptyState, clear, load };
}
