<script setup lang="ts">
import {
  Activity,
  AlertTriangle,
  ArrowLeft,
  Box,
  Check,
  Copy,
  GitBranch,
  Globe2,
  RefreshCw,
  ScrollText,
  Settings2,
  Trash2,
} from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import ServiceConfigurationPanel from "@/components/project/ServiceConfigurationPanel.vue";
import ServiceDetailPanel from "@/components/project/ServiceDetailPanel.vue";
import ServiceDomainsPanel from "@/components/project/ServiceDomainsPanel.vue";
import { Terminal } from "@/components/terminal";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";
import { useDeployment } from "@/composables/useDeployment";
import { useDeploymentStream } from "@/composables/useDeploymentStream";
import { useDomains } from "@/composables/useDomains";
import { useProjectEnvironment } from "@/composables/useProjectEnvironment";
import { useProviders } from "@/composables/useProviders";
import { useService } from "@/composables/useService";
import { useServiceRuntimeLogs } from "@/composables/useServiceRuntimeLogs";
import { useAuthStore } from "@/stores/auth";
import type {
  DeploymentEvent,
  DeploymentLog,
  DeploymentSummary,
  DomainSummary,
  ServiceInput,
  ServiceSummary,
} from "@/lib/types";

const route = useRoute();
const router = useRouter();
const { t } = useI18n();
const auth = useAuthStore();
const services = useService();
const deployments = useDeployment();
const domains = useDomains();
const projectEnvironment = useProjectEnvironment();
const providers = useProviders();
const serviceRuntimeLogs = useServiceRuntimeLogs();
const service = shallowRef<ServiceSummary | null>(null);
const serviceLoading = shallowRef(true);
const serviceError = shallowRef<string | null>(null);
type ServiceView = "configuration" | "domains" | "operations" | "logs";

const activeView = shallowRef<ServiceView>("configuration");
const viewTabs = computed(() => [
  { value: "configuration" as const, label: "Configuration", icon: Settings2 },
  { value: "domains" as const, label: "Domains", icon: Globe2 },
  { value: "operations" as const, label: "Deployments & logs", icon: Activity },
  ...(auth.isPlatformOperator
    ? [{ value: "logs" as const, label: t("serviceLogs.tab"), icon: ScrollText }]
    : []),
]);
const selectedDeploymentId = shallowRef<string | null>(null);
const streamLogs = shallowRef<DeploymentLog[]>([]);
const saving = shallowRef(false);
const rotatingAutoDeploySecret = shallowRef(false);
const deleteConfirmation = shallowRef(false);
const deleteConfirmName = shallowRef("");
const deleting = shallowRef(false);
const copiedServiceName = shallowRef(false);
let loadGeneration = 0;
let copyServiceNameTimer: number | undefined;

const projectId = computed(() => String(route.params.projectId));
const serviceId = computed(() => String(route.params.serviceId));
const deploymentData = deployments.data;
const deploymentSubmitting = deployments.submitting;
const deploymentError = deployments.error;
const environmentData = projectEnvironment.data;
const providerData = providers.data;
const serviceConfigError = services.error;
const latestDeployment = computed(
  () =>
    deploymentData.value.find((deployment) => deployment.service_id === service.value?.id) ?? null,
);
const runtimeLogDeployment = computed(
  () =>
    deploymentData.value.find(
      (deployment) =>
        deployment.service_id === service.value?.id && deployment.status === "healthy",
    ) ??
    deploymentData.value.find(
      (deployment) =>
        deployment.service_id === service.value?.id && deployment.status === "running",
    ) ??
    null,
);
const serviceLogEmptyMessage = computed(() => {
  if (serviceRuntimeLogs.emptyState.value === "no_container") {
    return t("serviceLogs.noContainer");
  }
  if (serviceRuntimeLogs.emptyState.value === "no_output") return t("serviceLogs.noOutput");
  return t("serviceLogs.noDeployment");
});
const serviceStatus = computed(() => {
  if (service.value?.source_config?.setup_required) return "setup";
  if (latestDeployment.value?.status === "healthy") return "healthy";
  if (["queued", "preparing", "running"].includes(latestDeployment.value?.status ?? "")) {
    return "live";
  }
  if (latestDeployment.value?.status === "failed") return "failed";
  return "inactive";
});
const serviceStatusLabel = computed(() => {
  if (serviceStatus.value === "setup") return "Setup required";
  if (serviceStatus.value === "inactive") return "Not deployed";
  return latestDeployment.value?.status ?? "Not deployed";
});
const canManage = computed(
  () => service.value?.role === "owner" || service.value?.role === "editor",
);
const sourceLabel = computed(() => {
  const current = service.value;
  if (!current) return "";
  if (current.source_config?.setup_required) return "Setup required";
  if (current.source_config?.source === "application") {
    return `${current.source_config.builder ?? "application"} · ${current.source_config.repository ?? "repository"}`;
  }
  if (current.source_config?.source === "template") {
    return `Template · ${current.source_config.template ?? "runtime"}`;
  }
  return "Compose file";
});

const stream = useDeploymentStream("", {
  onEvent: applyDeploymentEvent,
  onSnapshot: applyDeploymentSnapshot,
});
const logStream = useDeploymentStream("", {
  channel: "logs",
  onLog: (log) => {
    streamLogs.value = [...streamLogs.value, log].slice(-10_000);
  },
});
const streamConnected = stream.connected;
const logStreamConnected = logStream.connected;
const streamError = stream.error;
const logStreamError = logStream.error;
const projectRoute = computed(() => ({
  name: "ProjectDetail",
  params: { projectId: projectId.value },
}));

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
  streamLogs.value = [];
  stream.stop();
  logStream.stop();
  void stream.connect(deploymentId);
  void logStream.connect(deploymentId);
}

function loadServiceLogs() {
  const current = service.value;
  if (!current || !auth.isPlatformOperator) return;
  void serviceRuntimeLogs.load(current, runtimeLogDeployment.value);
}

async function load() {
  const generation = ++loadGeneration;
  serviceLoading.value = true;
  serviceError.value = null;
  service.value = null;
  domains.clear();
  deleteConfirmation.value = false;
  deleteConfirmName.value = "";
  copiedServiceName.value = false;
  deleting.value = false;
  selectedDeploymentId.value = null;
  streamLogs.value = [];
  serviceRuntimeLogs.clear();
  deployments.clear();
  stream.stop();
  logStream.stop();
  void projectEnvironment.load(projectId.value);
  void providers.load();

  try {
    const loaded = await services.get(serviceId.value);
    if (generation !== loadGeneration) return;
    if (!loaded) {
      serviceError.value = services.error.value ?? "Could not load service";
      toast.error("Service unavailable", {
        description: serviceError.value,
        action: { label: "Retry", onClick: () => void load() },
      });
      return;
    }
    service.value = loaded;
    activeView.value = loaded.source_config?.setup_required ? "configuration" : "operations";
    await Promise.all([deployments.load(loaded.id), domains.load([loaded.id])]);
    if (generation !== loadGeneration) return;
    const latest = deployments.data.value.find((deployment) => deployment.service_id === loaded.id);
    if (latest) selectDeployment(latest.id);
  } catch (cause) {
    if (generation !== loadGeneration) return;
    serviceError.value = cause instanceof Error ? cause.message : "Could not load service";
    toast.error("Service unavailable", {
      description: serviceError.value,
      action: { label: "Retry", onClick: () => void load() },
    });
  } finally {
    if (generation === loadGeneration) serviceLoading.value = false;
  }
}

async function saveConfiguration(input: ServiceInput) {
  const current = service.value;
  if (!current) return;
  saving.value = true;
  const updated = await services.update(current.id, input);
  saving.value = false;
  if (updated) {
    service.value = updated;
    toast.success("Service configuration saved", { description: updated.name });
    return;
  }
  toast.error("Could not save service configuration", {
    description: services.error.value ?? "Try again in a moment.",
  });
}

async function rotateAutoDeploySecret() {
  const current = service.value;
  if (!current) return;
  rotatingAutoDeploySecret.value = true;
  const secret = await services.rotateAutoDeploySecret(current.id);
  rotatingAutoDeploySecret.value = false;
  if (secret) {
    service.value = { ...current, auto_deploy_webhook_secret: secret };
    toast.success("Webhook secret rotated");
    return;
  }
  toast.error("Could not rotate webhook secret", {
    description: services.error.value ?? "Try again in a moment.",
  });
}

async function submitDeployment() {
  const current = service.value;
  if (!current || current.source_config?.setup_required) return;
  const deployment = await deployments.deploy(current.id);
  if (!deployment) {
    toast.error("Could not start deployment", {
      description: deploymentError.value ?? "Try again in a moment.",
    });
    return;
  }
  selectDeployment(deployment.id);
  toast.success("Deployment started");
}

async function stopService() {
  const current = service.value;
  if (!current) return;
  const deployment = await deployments.stop(current.id);
  if (!deployment) {
    toast.error("Could not stop deployment", {
      description: deploymentError.value ?? "Try again in a moment.",
    });
    return;
  }
  selectDeployment(deployment.id);
  toast.success("Stop requested");
}

async function cancelDeployment(deploymentId: string) {
  const deployment = await deployments.cancel(deploymentId);
  if (!deployment) {
    toast.error("Could not cancel deployment", {
      description: deploymentError.value ?? "Try again in a moment.",
    });
    return;
  }
  selectDeployment(deployment.id);
  toast.success("Deployment cancelled");
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

function requestDelete() {
  deleteConfirmation.value = true;
  deleteConfirmName.value = "";
  copiedServiceName.value = false;
}

function cancelDelete() {
  deleteConfirmation.value = false;
  deleteConfirmName.value = "";
  copiedServiceName.value = false;
  services.error.value = null;
}

async function copyServiceName() {
  const name = service.value?.name;
  if (!name) return;
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(name);
    } else {
      const input = document.createElement("textarea");
      input.value = name;
      input.setAttribute("readonly", "true");
      input.style.position = "fixed";
      input.style.opacity = "0";
      document.body.append(input);
      input.select();
      document.execCommand("copy");
      input.remove();
    }
  } catch {
    toast.error("Could not copy service name");
    return;
  }

  deleteConfirmName.value = name;
  copiedServiceName.value = true;
  toast.success("Service name copied");
  if (copyServiceNameTimer !== undefined) window.clearTimeout(copyServiceNameTimer);
  copyServiceNameTimer = window.setTimeout(() => {
    copiedServiceName.value = false;
    copyServiceNameTimer = undefined;
  }, 1_600);
}

async function deleteService() {
  const current = service.value;
  if (!current || deleteConfirmName.value !== current.name) return;
  deleting.value = true;
  const removed = await services.remove(current.id, deleteConfirmName.value);
  deleting.value = false;
  if (!removed) {
    toast.error("Could not delete service", {
      description: services.error.value ?? "Try again in a moment.",
    });
    return;
  }
  stream.stop();
  logStream.stop();
  serviceRuntimeLogs.clear();
  copiedServiceName.value = false;
  toast.success("Service deleted", { description: current.name });
  await router.push(projectRoute.value);
}

watch(
  () => `${projectId.value}:${serviceId.value}`,
  () => void load(),
  { immediate: true },
);

watch(
  () => [activeView.value, service.value?.id, runtimeLogDeployment.value?.id] as const,
  ([view]) => {
    if (view === "logs") loadServiceLogs();
  },
);

onUnmounted(() => {
  stream.stop();
  logStream.stop();
  serviceRuntimeLogs.clear();
  if (copyServiceNameTimer !== undefined) window.clearTimeout(copyServiceNameTimer);
});
</script>

<template>
  <div class="app-page">
    <RouterLink
      class="group inline-flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
      :to="projectRoute"
    >
      <ArrowLeft
        class="size-3.5 transition-transform group-hover:-translate-x-0.5"
        :stroke-width="1.5"
      />
      Back to project
    </RouterLink>

    <section
      v-if="serviceLoading"
      class="mt-6 rounded-[10px] border border-border bg-card px-5 py-6"
      role="status"
    >
      <div class="flex items-center gap-3">
        <Skeleton class="size-11 shrink-0 rounded-[5px]" />
        <div class="grid flex-1 gap-2">
          <Skeleton class="h-5 w-48 max-w-full" />
          <Skeleton class="h-2.5 w-32 max-w-full" />
        </div>
      </div>
    </section>

    <template v-else-if="service">
      <header
        class="mt-6 grid gap-5 border-b border-border pb-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end"
      >
        <div class="flex min-w-0 items-start gap-3">
          <div
            class="grid size-12 shrink-0 place-items-center rounded-[8px] border border-border bg-card text-muted-foreground"
          >
            <GitBranch
              v-if="service.source_config?.source === 'application'"
              :size="20"
              :stroke-width="1.5"
            />
            <Box v-else :size="20" :stroke-width="1.5" />
          </div>
          <div class="min-w-0">
            <p class="ui-label">Service</p>
            <h1 class="mt-2 truncate text-3xl leading-none font-normal">{{ service.name }}</h1>
            <p class="mt-2 truncate font-mono text-xs text-muted-foreground">{{ sourceLabel }}</p>
            <p class="mt-2 flex items-center gap-2 text-xs text-muted-foreground">
              <span
                class="status-dot"
                :data-status="serviceStatus === 'setup' ? 'warning' : serviceStatus"
                aria-hidden="true"
              />
              <span class="capitalize">{{ serviceStatusLabel }}</span>
            </p>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-3 lg:justify-end">
          <Tabs
            :model-value="activeView"
            class="min-w-0 max-[480px]:w-full"
            @update:model-value="(value) => (activeView = value as ServiceView)"
          >
            <TabsList class="h-9 max-w-full rounded-[4px] max-[480px]:w-full">
              <TabsTrigger
                v-for="tab in viewTabs"
                :key="tab.value"
                :value="tab.value"
                class="min-w-0 px-3 text-xs max-[480px]:flex-1"
              >
                <component :is="tab.icon" class="size-3.5" :stroke-width="1.5" />
                <span class="truncate">{{ tab.label }}</span>
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <Button v-if="canManage" size="sm" variant="destructive" @click="requestDelete">
            <Trash2 data-icon="inline-start" :stroke-width="1.5" />
            Delete
          </Button>
        </div>
      </header>

      <main class="mt-6 grid gap-6">
        <ServiceConfigurationPanel
          v-if="activeView === 'configuration'"
          :error="serviceConfigError"
          :inherited-variables="environmentData.variables"
          :providers="providerData"
          :rotating-auto-deploy-secret="rotatingAutoDeploySecret"
          :saving="saving"
          :service="service"
          @save="saveConfiguration"
          @rotate-auto-deploy-secret="rotateAutoDeploySecret"
        />
        <ServiceDomainsPanel
          v-else-if="activeView === 'domains'"
          :can-manage="canManage"
          :domains="domains.data.value"
          :error="domains.error.value"
          :fixed-service-id="service.id"
          :loading="domains.loading.value"
          :services="[service]"
          @create="(domainServiceId, hostname) => domains.create(domainServiceId, hostname)"
          @remove="(domain: DomainSummary) => domains.remove(domain)"
          @retry="domains.load([service.id])"
          @verify="(domain: DomainSummary) => domains.verify(domain)"
        />
        <ServiceDetailPanel
          v-else-if="activeView === 'operations'"
          :can-manage="canManage"
          :connected="streamConnected && logStreamConnected"
          :deployments="deploymentData"
          :hide-header="true"
          :hide-config="true"
          :logs="streamLogs"
          :selected-deployment-id="selectedDeploymentId"
          :service="service"
          :stream-error="deploymentError ?? streamError ?? logStreamError"
          :submitting="deploymentSubmitting"
          @deploy="submitDeployment"
          @cancel="cancelDeployment"
          @rollback="rollbackDeployment"
          @select-deployment="selectDeployment"
          @stop="stopService"
        />
        <section v-else-if="activeView === 'logs'" class="grid gap-4">
          <header
            class="flex flex-wrap items-start justify-between gap-4 border-b border-border pb-4"
          >
            <div class="min-w-0">
              <p class="ui-label">{{ t("serviceLogs.output") }}</p>
              <h2 class="mt-2 text-xl leading-none font-normal">{{ t("serviceLogs.title") }}</h2>
              <p class="mt-2 text-xs text-muted-foreground">{{ t("serviceLogs.latest") }}</p>
              <p
                v-if="serviceRuntimeLogs.container.value"
                class="mt-2 truncate font-mono text-[11px] text-muted-foreground"
              >
                {{ serviceRuntimeLogs.container.value.name }}
              </p>
            </div>
            <Button
              size="sm"
              variant="outline"
              :disabled="serviceRuntimeLogs.loading.value"
              @click="loadServiceLogs"
            >
              <RefreshCw
                data-icon="inline-start"
                :class="serviceRuntimeLogs.loading.value ? 'animate-spin' : ''"
                :stroke-width="1.5"
              />
              {{ t("serviceLogs.refresh") }}
            </Button>
          </header>
          <p
            v-if="serviceRuntimeLogs.error.value"
            class="border border-destructive/40 px-3 py-2 text-xs text-destructive"
            role="alert"
          >
            {{ serviceRuntimeLogs.error.value }}
          </p>
          <p
            v-else-if="serviceRuntimeLogs.loading.value"
            class="py-8 text-center text-xs text-muted-foreground"
            role="status"
          >
            {{ t("serviceLogs.loading") }}
          </p>
          <Terminal
            v-else-if="serviceRuntimeLogs.output.value"
            :copy-label="t('serviceLogs.copy')"
            :output="serviceRuntimeLogs.output.value"
            :title="t('serviceLogs.title')"
          />
          <p v-else class="py-8 text-sm text-muted-foreground" role="status">
            {{ serviceLogEmptyMessage }}
          </p>
        </section>
        <AlertDialogRoot v-model:open="deleteConfirmation">
          <AlertDialogPortal>
            <AlertDialogOverlay class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm" />
            <AlertDialogContent
              class="fixed top-1/2 left-1/2 z-50 grid w-[calc(100%-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2 gap-5 rounded-[10px] border border-border bg-card p-6 shadow-none"
            >
              <div class="flex items-start gap-3">
                <div
                  class="grid size-9 shrink-0 place-items-center rounded-[6px] border border-destructive/30 bg-destructive/10 text-destructive"
                >
                  <AlertTriangle class="size-4" :stroke-width="1.5" />
                </div>
                <div class="min-w-0">
                  <AlertDialogTitle class="text-base font-medium">Delete service?</AlertDialogTitle>
                  <AlertDialogDescription class="mt-2 text-sm leading-5">
                    This permanently removes
                    <span class="inline-flex max-w-full items-center gap-1 align-bottom">
                      <span class="max-w-[18rem] truncate font-medium text-foreground">{{
                        service.name
                      }}</span>
                      <Button
                        variant="ghost"
                        class="grid size-5 shrink-0 place-items-center rounded-[3px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                        type="button"
                        :aria-label="
                          copiedServiceName
                            ? 'Service name copied and filled'
                            : 'Copy and fill service name'
                        "
                        :title="
                          copiedServiceName ? 'Copied and filled' : 'Copy and fill service name'
                        "
                        @click="copyServiceName"
                      >
                        <Check v-if="copiedServiceName" class="size-3" :stroke-width="1.75" />
                        <Copy v-else class="size-3" :stroke-width="1.5" />
                      </Button>
                    </span>
                    and its configuration, deployments, logs, and domains.
                  </AlertDialogDescription>
                </div>
              </div>
              <Label class="grid gap-2 text-xs text-muted-foreground" for="delete-service-name">
                Confirm service name
                <Input
                  id="delete-service-name"
                  v-model="deleteConfirmName"
                  :placeholder="service.name"
                  autocomplete="off"
                  :disabled="deleting"
                />
              </Label>
              <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
                <AlertDialogCancel as-child>
                  <Button
                    class="w-full sm:w-auto"
                    variant="outline"
                    :disabled="deleting"
                    @click="cancelDelete"
                  >
                    Cancel
                  </Button>
                </AlertDialogCancel>
                <AlertDialogAction as-child>
                  <Button
                    class="w-full sm:w-auto"
                    variant="destructive"
                    :disabled="deleteConfirmName !== service.name || deleting"
                    @click.prevent="deleteService"
                  >
                    <Trash2 class="size-4" :stroke-width="1.5" />
                    {{ deleting ? "Deleting..." : "Delete service" }}
                  </Button>
                </AlertDialogAction>
              </div>
            </AlertDialogContent>
          </AlertDialogPortal>
        </AlertDialogRoot>
      </main>
    </template>
  </div>
</template>
