<script setup lang="ts">
import { ArrowLeft, Box, GitBranch, RefreshCw } from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute } from "vue-router";
import ServiceConfigurationPanel from "@/components/project/ServiceConfigurationPanel.vue";
import ServiceDetailPanel from "@/components/project/ServiceDetailPanel.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
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
const services = useService();
const deployments = useDeployment();
const projectEnvironment = useProjectEnvironment();
const providers = useProviders();
const service = shallowRef<ServiceSummary | null>(null);
const serviceLoading = shallowRef(true);
const serviceError = shallowRef<string | null>(null);
const activeView = shallowRef<"configuration" | "operations">("configuration");
const selectedDeploymentId = shallowRef<string | null>(null);
const streamLogs = shallowRef<DeploymentLog[]>([]);
const saving = shallowRef(false);
let loadGeneration = 0;

const projectId = computed(() => String(route.params.projectId));
const serviceId = computed(() => String(route.params.serviceId));
const deploymentData = deployments.data;
const deploymentSubmitting = deployments.submitting;
const deploymentError = deployments.error;
const environmentData = projectEnvironment.data;
const providerData = providers.data;
const serviceConfigError = services.error;
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
  <div class="w-full max-w-[1200px]">
    <RouterLink
      class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
      :to="projectRoute"
    >
      <ArrowLeft :size="15" :stroke-width="1.5" />
      Back to project
    </RouterLink>

    <section
      v-if="serviceLoading"
      class="mt-[22px] border border-border bg-card px-5 py-6"
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
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-8"
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
        class="mt-[22px] flex items-start justify-between gap-6 border-b border-border pb-5 max-[640px]:flex-col"
      >
        <div class="flex min-w-0 items-start gap-3">
          <div
            class="grid size-11 shrink-0 place-items-center rounded-[5px] border border-border bg-muted text-muted-foreground"
          >
            <GitBranch
              v-if="service.source_config?.source === 'application'"
              :size="20"
              :stroke-width="1.5"
            />
            <Box v-else :size="20" :stroke-width="1.5" />
          </div>
          <div class="min-w-0">
            <p class="ui-label">Service detail</p>
            <h1 class="mt-2 truncate text-[29px] leading-none font-normal">{{ service.name }}</h1>
            <p class="mt-2 truncate font-mono text-xs text-muted-foreground">{{ sourceLabel }}</p>
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button
            v-for="tab in [
              { value: 'configuration', label: 'Configuration' },
              { value: 'operations', label: 'Deployments & logs' },
            ]"
            :key="tab.value"
            size="sm"
            :variant="activeView === tab.value ? 'default' : 'outline'"
            @click="activeView = tab.value as typeof activeView"
          >
            {{ tab.label }}
          </Button>
        </div>
      </header>

      <main class="mt-[22px] grid gap-4">
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
          :hide-config="true"
          :logs="streamLogs"
          :selected-deployment-id="selectedDeploymentId"
          :service="service"
          :stream-error="deploymentError ?? streamError ?? logStreamError"
          :submitting="deploymentSubmitting"
          @deploy="submitDeployment"
          @edit="activeView = 'configuration'"
          @rollback="rollbackDeployment"
          @select-deployment="selectDeployment"
          @stop="stopService"
        />
      </main>
    </template>
  </div>
</template>
