<script setup lang="ts">
import { Activity, ArrowLeft, Box, GitBranch, RefreshCw, Settings2, Trash2 } from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import ServiceConfigurationPanel from "@/components/project/ServiceConfigurationPanel.vue";
import ServiceDetailPanel from "@/components/project/ServiceDetailPanel.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDeployment } from "@/composables/useDeployment";
import { useDeploymentStream } from "@/composables/useDeploymentStream";
import { useProjectEnvironment } from "@/composables/useProjectEnvironment";
import { useProviders } from "@/composables/useProviders";
import { useService } from "@/composables/useService";
import type {
  DeploymentEvent,
  DeploymentLog,
  DeploymentSummary,
  ServiceInput,
  ServiceSummary,
} from "@/lib/types";

const route = useRoute();
const router = useRouter();
const services = useService();
const deployments = useDeployment();
const projectEnvironment = useProjectEnvironment();
const providers = useProviders();
const service = shallowRef<ServiceSummary | null>(null);
const serviceLoading = shallowRef(true);
const serviceError = shallowRef<string | null>(null);
const activeView = shallowRef<"configuration" | "operations">("configuration");
const viewTabs = [
  { value: "configuration" as const, label: "Configuration", icon: Settings2 },
  { value: "operations" as const, label: "Deployments & logs", icon: Activity },
];
const selectedDeploymentId = shallowRef<string | null>(null);
const streamLogs = shallowRef<DeploymentLog[]>([]);
const saving = shallowRef(false);
const deleteConfirmation = shallowRef(false);
const deleteConfirmName = shallowRef("");
const deleting = shallowRef(false);
let loadGeneration = 0;

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

async function load() {
  const generation = ++loadGeneration;
  serviceLoading.value = true;
  serviceError.value = null;
  service.value = null;
  deleteConfirmation.value = false;
  deleteConfirmName.value = "";
  deleting.value = false;
  selectedDeploymentId.value = null;
  streamLogs.value = [];
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
      return;
    }
    service.value = loaded;
    activeView.value = loaded.source_config?.setup_required ? "configuration" : "operations";
    await deployments.load(loaded.id);
    if (generation !== loadGeneration) return;
    const latest = deployments.data.value.find((deployment) => deployment.service_id === loaded.id);
    if (latest) selectDeployment(latest.id);
  } catch (cause) {
    if (generation !== loadGeneration) return;
    serviceError.value = cause instanceof Error ? cause.message : "Could not load service";
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
  if (updated) service.value = updated;
}

async function submitDeployment() {
  const current = service.value;
  if (!current || current.source_config?.setup_required) return;
  const deployment = await deployments.deploy(current.id);
  if (deployment) selectDeployment(deployment.id);
}

async function stopService() {
  const current = service.value;
  if (!current) return;
  const deployment = await deployments.stop(current.id);
  if (deployment) selectDeployment(deployment.id);
}

async function rollbackDeployment(deploymentId: string) {
  const deployment = await deployments.rollback(deploymentId);
  if (deployment) selectDeployment(deployment.id);
}

function requestDelete() {
  deleteConfirmation.value = true;
  deleteConfirmName.value = "";
}

function cancelDelete() {
  deleteConfirmation.value = false;
  deleteConfirmName.value = "";
  services.error.value = null;
}

async function deleteService() {
  const current = service.value;
  if (!current || deleteConfirmName.value !== current.name) return;
  deleting.value = true;
  const removed = await services.remove(current.id, deleteConfirmName.value);
  deleting.value = false;
  if (!removed) return;
  stream.stop();
  logStream.stop();
  await router.push(projectRoute.value);
}

watch(
  () => `${projectId.value}:${serviceId.value}`,
  () => void load(),
  { immediate: true },
);

onUnmounted(() => {
  stream.stop();
  logStream.stop();
});
</script>

<template>
  <div class="w-full max-w-[1200px] pb-10">
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

    <section
      v-else-if="serviceError || !service"
      class="mt-6 rounded-[10px] border border-destructive/40 bg-card px-5 py-8"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ serviceError ?? "Service not found" }}</p>
      <Button class="mt-4" variant="outline" size="sm" @click="load">
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Retry
      </Button>
    </section>

    <template v-else>
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
            @update:model-value="(value) => (activeView = value as 'configuration' | 'operations')"
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
        <section
          v-if="deleteConfirmation"
          class="rounded-[10px] border border-destructive/40 bg-destructive/[0.04] p-5"
          role="alertdialog"
          aria-labelledby="service-delete-title"
        >
          <p id="service-delete-title" class="text-sm font-medium">Delete {{ service.name }}?</p>
          <p class="mt-1 text-xs text-muted-foreground">
            Type the service name to permanently remove its configuration, deployments, logs, and
            domains.
          </p>
          <Input
            v-model="deleteConfirmName"
            class="mt-3"
            :placeholder="service.name"
            autocomplete="off"
            :disabled="deleting"
          />
          <p v-if="serviceConfigError" class="mt-2 text-xs text-destructive" role="alert">
            {{ serviceConfigError }}
          </p>
          <div class="mt-3 flex flex-wrap gap-2">
            <Button
              size="sm"
              variant="destructive"
              :disabled="deleteConfirmName !== service.name || deleting"
              @click="deleteService"
            >
              {{ deleting ? "Deleting..." : "Delete service" }}
            </Button>
            <Button size="sm" variant="outline" :disabled="deleting" @click="cancelDelete">
              Cancel
            </Button>
          </div>
        </section>
        <ServiceConfigurationPanel
          v-if="activeView === 'configuration'"
          :error="serviceConfigError"
          :inherited-variables="environmentData.variables"
          :providers="providerData"
          :saving="saving"
          :service="service"
          @save="saveConfiguration"
        />
        <ServiceDetailPanel
          v-else
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
          @rollback="rollbackDeployment"
          @select-deployment="selectDeployment"
          @stop="stopService"
        />
      </main>
    </template>
  </div>
</template>
