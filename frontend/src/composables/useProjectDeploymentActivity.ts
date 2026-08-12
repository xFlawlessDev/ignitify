import {
  computed,
  nextTick,
  onUnmounted,
  shallowRef,
  watch,
  type ComponentPublicInstance,
  type Ref,
} from "vue";
import { toast } from "vue-sonner";
import { useDeployment } from "@/composables/useDeployment";
import { useDeploymentStream } from "@/composables/useDeploymentStream";
import { useProjectActivity } from "@/composables/useProjectActivity";
import { useService } from "@/composables/useService";
import type { DeploymentEvent, DeploymentLog, DeploymentSummary } from "@/lib/types";

const SERVICES_PER_PAGE = 6;
const DEPLOYMENTS_PER_PAGE = 5;

export function useProjectDeploymentActivity(activeTab: Ref<string>) {
  const services = useService();
  const deployments = useDeployment();
  const activity = useProjectActivity();
  const selectedDeploymentId = shallowRef<string | null>(null);
  const deploymentLogsAnchor = shallowRef<HTMLElement | null>(null);
  const streamLogs = shallowRef<DeploymentLog[]>([]);
  const logStream = useDeploymentStream("", {
    channel: "logs",
    onLog: (log) => {
      streamLogs.value = [...streamLogs.value, log].slice(-10_000);
    },
  });
  const stream = useDeploymentStream("", {
    onEvent: applyDeploymentEvent,
    onSnapshot: applyDeploymentSnapshot,
  });
  const serviceData = services.data;
  const serviceError = services.error;
  const serviceLoading = services.loading;
  const serviceCurrentPage = shallowRef(1);
  const serviceViewMode = shallowRef<"list" | "catalog">("catalog");
  const serviceCount = computed(() => serviceData.value.length);
  const servicePageCount = computed(() =>
    Math.max(1, Math.ceil(serviceCount.value / SERVICES_PER_PAGE)),
  );
  const visibleServices = computed(() => {
    const start = (serviceCurrentPage.value - 1) * SERVICES_PER_PAGE;
    return serviceData.value.slice(start, start + SERVICES_PER_PAGE);
  });
  const firstVisibleService = computed(() =>
    serviceCount.value === 0 ? 0 : (serviceCurrentPage.value - 1) * SERVICES_PER_PAGE + 1,
  );
  const lastVisibleService = computed(() =>
    Math.min(serviceCurrentPage.value * SERVICES_PER_PAGE, serviceCount.value),
  );
  const activityData = activity.data;
  const activityError = activity.error;
  const activityLoading = activity.loading;
  const deploymentData = deployments.data;
  const deploymentError = deployments.error;
  const deploymentLoading = deployments.loading;
  const deploymentSubmitting = deployments.submitting;
  const availableDeployments = computed(() =>
    [...deploymentData.value].sort(
      (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
    ),
  );
  const deploymentCurrentPage = shallowRef(1);
  const deploymentCount = computed(() => availableDeployments.value.length);
  const deploymentPageCount = computed(() =>
    Math.max(1, Math.ceil(deploymentCount.value / DEPLOYMENTS_PER_PAGE)),
  );
  const visibleDeployments = computed(() => {
    const start = (deploymentCurrentPage.value - 1) * DEPLOYMENTS_PER_PAGE;
    return availableDeployments.value.slice(start, start + DEPLOYMENTS_PER_PAGE);
  });
  const firstVisibleDeployment = computed(() =>
    deploymentCount.value === 0 ? 0 : (deploymentCurrentPage.value - 1) * DEPLOYMENTS_PER_PAGE + 1,
  );
  const lastVisibleDeployment = computed(() =>
    Math.min(deploymentCurrentPage.value * DEPLOYMENTS_PER_PAGE, deploymentCount.value),
  );
  const selectedDeployment = computed(() =>
    availableDeployments.value.find((deployment) => deployment.id === selectedDeploymentId.value),
  );
  let loadGeneration = 0;

  watch(
    servicePageCount,
    (count) => {
      if (serviceCurrentPage.value > count) serviceCurrentPage.value = count;
    },
    { immediate: true },
  );

  watch(
    deploymentPageCount,
    (count) => {
      if (deploymentCurrentPage.value > count) deploymentCurrentPage.value = count;
    },
    { immediate: true },
  );

  watch(
    availableDeployments,
    (items) => {
      if (!items.length) {
        selectedDeploymentId.value = null;
        streamLogs.value = [];
        stream.stop();
        logStream.stop();
        return;
      }
      if (!items.some((deployment) => deployment.id === selectedDeploymentId.value)) {
        selectDeployment(items[0].id);
      }
    },
    { immediate: true },
  );

  watch(activeTab, (tab) => {
    if (tab === "deployments" && selectedDeploymentId.value) {
      void stream.connect(selectedDeploymentId.value);
      void logStream.connect(selectedDeploymentId.value);
      return;
    }
    stream.stop();
    logStream.stop();
  });

  function setServiceViewMode(mode: "list" | "catalog") {
    serviceViewMode.value = mode;
    serviceCurrentPage.value = 1;
  }

  function goToPreviousServicePage() {
    serviceCurrentPage.value = Math.max(1, serviceCurrentPage.value - 1);
  }

  function goToNextServicePage() {
    serviceCurrentPage.value = Math.min(servicePageCount.value, serviceCurrentPage.value + 1);
  }

  function goToPreviousDeploymentPage() {
    deploymentCurrentPage.value = Math.max(1, deploymentCurrentPage.value - 1);
  }

  function goToNextDeploymentPage() {
    deploymentCurrentPage.value = Math.min(
      deploymentPageCount.value,
      deploymentCurrentPage.value + 1,
    );
  }

  function applyDeploymentEvent(event: DeploymentEvent) {
    deployments.data.value = deployments.data.value.map((deployment) =>
      deployment.id === event.deployment_id && event.kind.startsWith("deployment.")
        ? {
            ...deployment,
            status: event.kind.slice("deployment.".length) as DeploymentSummary["status"],
            failure_reason:
              (event.payload.failure_reason as string | null | undefined) ??
              deployment.failure_reason,
          }
        : deployment,
    );
  }

  function applyDeploymentSnapshot(deployment: DeploymentSummary) {
    deployments.data.value = deployments.data.value.map((item) =>
      item.id === deployment.id ? deployment : item,
    );
  }

  function selectDeployment(deploymentId: string) {
    selectedDeploymentId.value = deploymentId;
    const deploymentIndex = availableDeployments.value.findIndex(
      (deployment) => deployment.id === deploymentId,
    );
    if (deploymentIndex >= 0) {
      deploymentCurrentPage.value = Math.floor(deploymentIndex / DEPLOYMENTS_PER_PAGE) + 1;
    }
    streamLogs.value = [];
    stream.stop();
    logStream.stop();
    if (activeTab.value === "deployments") {
      void stream.connect(deploymentId);
      void logStream.connect(deploymentId);
    }
  }

  function selectDeploymentAndRevealLogs(deploymentId: string) {
    selectDeployment(deploymentId);
    if (!window.matchMedia("(max-width: 1023px)").matches) return;

    void nextTick(() => {
      const behavior = window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth";
      deploymentLogsAnchor.value?.scrollIntoView({ behavior, block: "start" });
    });
  }

  function setDeploymentLogsAnchor(element: Element | ComponentPublicInstance | null) {
    deploymentLogsAnchor.value = element instanceof HTMLElement ? element : null;
  }

  async function loadProjectWorkloads(projectId: string) {
    const generation = ++loadGeneration;
    deployments.clear();
    deploymentCurrentPage.value = 1;
    await services.load(projectId);
    if (generation !== loadGeneration) return;
    if (services.error.value) {
      toast.error("Services unavailable", { description: services.error.value });
    }
    await deployments.loadProject(projectId);
    if (generation !== loadGeneration) return;
    if (deploymentError.value) {
      toast.error("Deployments unavailable", { description: deploymentError.value });
    }
    void activity.load(projectId);
  }

  async function submitDeployment(serviceId: string) {
    const deployment = await deployments.deploy(serviceId);
    if (!deployment) {
      toast.error("Could not start deployment", {
        description: deploymentError.value ?? "Try again in a moment.",
      });
      return;
    }
    selectDeployment(deployment.id);
    toast.success("Deployment started");
  }

  async function stopDeployment(serviceId: string) {
    const deployment = await deployments.stop(serviceId);
    if (!deployment) {
      toast.error("Could not stop deployment", {
        description: deploymentError.value ?? "Try again in a moment.",
      });
      return;
    }
    selectDeployment(deployment.id);
    toast.success("Stop requested");
  }

  async function rollbackDeployment(deploymentId: string) {
    const deployment = await deployments.rollback(deploymentId);
    if (!deployment) {
      toast.error("Could not roll back deployment", {
        description: deploymentError.value ?? "Try again in a moment.",
      });
      return;
    }
    selectDeployment(deployment.id);
    toast.success("Rollback started");
  }

  onUnmounted(() => {
    stream.stop();
    logStream.stop();
  });

  return {
    activity,
    activityData,
    activityError,
    activityLoading,
    availableDeployments,
    deploymentCount,
    deploymentCurrentPage,
    deploymentData,
    deploymentError,
    deploymentLoading,
    deploymentPageCount,
    deploymentSubmitting,
    firstVisibleDeployment,
    firstVisibleService,
    goToNextDeploymentPage,
    goToNextServicePage,
    goToPreviousDeploymentPage,
    goToPreviousServicePage,
    lastVisibleDeployment,
    lastVisibleService,
    loadProjectWorkloads,
    rollbackDeployment,
    selectedDeployment,
    selectedDeploymentId,
    selectDeployment,
    selectDeploymentAndRevealLogs,
    serviceCurrentPage,
    serviceCount,
    serviceData,
    serviceError,
    serviceLoading,
    servicePageCount,
    serviceViewMode,
    services,
    setDeploymentLogsAnchor,
    setServiceViewMode,
    stopDeployment,
    stream,
    streamLogs,
    logStream,
    submitDeployment,
    visibleDeployments,
    visibleServices,
  };
}
