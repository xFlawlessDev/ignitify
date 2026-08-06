<script setup lang="ts">
import { ArrowLeft, Box, Pencil, RefreshCw } from "@lucide/vue";
import { onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute } from "vue-router";
import DeploymentLogsPanel from "@/components/project/DeploymentLogsPanel.vue";
import ProjectActivityPanel from "@/components/project/ProjectActivityPanel.vue";
import ProjectServiceList from "@/components/project/ProjectServiceList.vue";
import ServiceDomainsPanel from "@/components/project/ServiceDomainsPanel.vue";
import ServiceDialog from "@/components/project/ServiceDialog.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useProject } from "@/composables/useProject";
import { useProjectActivity } from "@/composables/useProjectActivity";
import ProjectDeploymentTimeline from "@/components/project/ProjectDeploymentTimeline.vue";
import { useDeployment } from "@/composables/useDeployment";
import { useDeploymentStream } from "@/composables/useDeploymentStream";
import { useDomains } from "@/composables/useDomains";
import { useService } from "@/composables/useService";
import type {
  DeploymentEvent,
  DeploymentLog,
  DeploymentSummary,
  DomainSummary,
  ServiceInput,
  ServiceSummary,
} from "@/lib/types";

const route = useRoute();
const { data, error, load: fetchProject, loading, update } = useProject();
const services = useService();
const deployments = useDeployment();
const domains = useDomains();
const activity = useProjectActivity();
const selectedDeploymentId = shallowRef<string | null>(null);
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
const deploymentData = deployments.data;
const deploymentError = deployments.error;
const deploymentLoading = deployments.loading;
const deploymentSubmitting = deployments.submitting;
const activeTab = shallowRef("overview");
const editName = shallowRef("");
const serviceDialogOpen = shallowRef(false);
const selectedService = shallowRef<ServiceSummary | null>(null);
const savingService = shallowRef(false);
let projectLoadGeneration = 0;
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
  if (activeTab.value === "deployments") {
    void stream.connect(deploymentId);
    void logStream.connect(deploymentId);
  }
}

function load(projectId: string) {
  const generation = ++projectLoadGeneration;
  deployments.clear();
  selectedService.value = null;
  serviceDialogOpen.value = false;
  void fetchProject(projectId).then(() => {
    if (generation !== projectLoadGeneration) return;
    editName.value = data.value?.name ?? "";
    if (data.value) {
      void loadDeployments(data.value.id, generation);
      void activity.load(data.value.id);
    }
  });
}

async function renameProject() {
  if (!editName.value.trim()) return;
  await update({ name: editName.value });
}

function createService() {
  selectedService.value = null;
  serviceDialogOpen.value = true;
}

function editService(service: ServiceSummary) {
  selectedService.value = service;
  serviceDialogOpen.value = true;
}

async function loadDeployments(projectId: string, generation = projectLoadGeneration) {
  await services.load(projectId);
  if (generation !== projectLoadGeneration) return;
  await Promise.all([
    deployments.loadProject(projectId),
    domains.load(services.data.value.map((service) => service.id)),
  ]);
}

async function submitDeployment(serviceId: string) {
  const deployment = await deployments.deploy(serviceId);
  if (deployment) {
    activeTab.value = "deployments";
    selectDeployment(deployment.id);
  }
}

async function stopDeployment(serviceId: string) {
  await deployments.stop(serviceId);
}

async function rollbackDeployment(deploymentId: string) {
  await deployments.rollback(deploymentId);
}

async function saveService(input: ServiceInput) {
  if (!data.value) return;
  savingService.value = true;
  const service = selectedService.value
    ? await services.update(selectedService.value.id, input)
    : await services.create(data.value.id, input);
  savingService.value = false;
  if (service) serviceDialogOpen.value = false;
}

watch(() => String(route.params.projectId), load, { immediate: true });
watch(activeTab, (tab) => {
  if (tab === "deployments" && selectedDeploymentId.value) {
    void stream.connect(selectedDeploymentId.value);
    void logStream.connect(selectedDeploymentId.value);
  } else {
    stream.stop();
    logStream.stop();
  }
});
onUnmounted(() => {
  stream.stop();
  logStream.stop();
});
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <RouterLink
      class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
      to="/projects"
    >
      <ArrowLeft :size="15" :stroke-width="1.5" />
      Projects
    </RouterLink>

    <p
      v-if="loading"
      class="mt-[22px] border border-border bg-card px-5 py-8 text-sm text-muted-foreground"
      role="status"
    >
      Loading project...
    </p>
    <section
      v-else-if="error"
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-8"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button
        class="mt-4"
        variant="outline"
        size="sm"
        @click="load(String(route.params.projectId))"
      >
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Retry
      </Button>
    </section>
    <template v-else-if="data">
      <header
        class="mt-[22px] flex items-center justify-between gap-6 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
      >
        <div class="flex min-w-0 items-center gap-[13px]">
          <div
            class="grid size-11 shrink-0 place-items-center rounded-[5px] border border-border bg-muted text-muted-foreground"
          >
            <Box :size="20" :stroke-width="1.5" />
          </div>
          <div class="min-w-0">
            <h1 class="m-0 truncate text-[29px] leading-none font-normal">
              {{ data.name }}
            </h1>
            <p class="mt-2 truncate text-xs text-muted-foreground">
              {{ data.default_environment.name }} environment
            </p>
          </div>
        </div>
      </header>

      <nav
        class="mt-6 flex h-[39px] min-w-0 gap-1 overflow-x-auto border-b border-border"
        aria-label="Project sections"
      >
        <button
          v-for="tab in [
            'overview',
            'services',
            'domains',
            'deployments',
            'activity',
            'settings',
          ]"
          :key="tab"
          class="h-[39px] flex-none border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground capitalize hover:text-foreground"
          :class="activeTab === tab ? 'border-b-[var(--status-live)] text-foreground' : ''"
          type="button"
          :aria-current="activeTab === tab ? 'page' : undefined"
          @click="activeTab = tab"
        >
          {{ tab }}
        </button>
      </nav>

      <section
        v-if="activeTab === 'overview'"
        class="mt-[22px] grid min-w-0 overflow-hidden border border-border bg-card sm:grid-cols-2"
      >
        <div class="grid gap-2 border-b border-border p-5 sm:border-r sm:border-b-0">
          <p class="ui-label">Default environment</p>
          <strong class="text-[15px] font-medium">{{ data.default_environment.name }}</strong>
          <span class="text-xs text-muted-foreground">Ready for service configuration.</span>
        </div>
        <div class="grid gap-2 p-5">
          <p class="ui-label">Access</p>
          <strong class="text-[15px] font-medium capitalize">{{ data.role }}</strong>
          <span class="text-xs text-muted-foreground"
            >Project membership controls service configuration.</span
          >
        </div>
      </section>

      <section v-else-if="activeTab === 'services'" class="mt-[22px] grid gap-4">
        <p v-if="services.loading" class="text-sm text-muted-foreground" role="status">
          Loading services...
        </p>
        <section
          v-else-if="services.error"
          class="border border-destructive/40 bg-card px-5 py-4"
          role="alert"
        >
          <p class="text-sm text-destructive">{{ services.error }}</p>
          <Button class="mt-3" size="sm" variant="outline" @click="services.load(data.id)"
            >Retry</Button
          >
        </section>
        <ProjectServiceList
          v-else
          :can-manage="data.role === 'owner' || data.role === 'editor'"
          :services="serviceData"
          @create="createService"
          @edit="editService"
        />
      </section>

      <ServiceDomainsPanel
        v-else-if="activeTab === 'domains'"
        class="mt-[22px]"
        :can-manage="data.role === 'owner' || data.role === 'editor'"
        :domains="domains.data.value"
        :error="domains.error.value"
        :loading="domains.loading.value"
        :services="serviceData"
        @create="(serviceId, hostname) => domains.create(serviceId, hostname)"
        @remove="(domain: DomainSummary) => domains.remove(domain)"
        @retry="domains.load(serviceData.map((service) => service.id))"
      />

      <section v-else-if="activeTab === 'deployments'" class="mt-[22px] grid gap-4">
        <ProjectDeploymentTimeline
          :deployments="deploymentData"
          :error="deploymentError"
          :loading="deploymentLoading"
          :services="serviceData"
          :submitting="deploymentSubmitting"
          @deploy="submitDeployment"
          @stop="stopDeployment"
          @retry="loadDeployments(data.id)"
          @rollback="rollbackDeployment"
        />
        <div v-if="deploymentData.length" class="flex flex-wrap gap-2">
          <Button
            v-for="deployment in deploymentData"
            :key="deployment.id"
            size="sm"
            :variant="selectedDeploymentId === deployment.id ? 'default' : 'outline'"
            @click="selectDeployment(deployment.id)"
            >g{{ deployment.generation }}</Button
          >
        </div>
        <DeploymentLogsPanel
          v-if="selectedDeploymentId"
          :connected="stream.connected.value && logStream.connected.value"
          :logs="streamLogs"
          :stream-error="stream.error.value ?? logStream.error.value"
        />
      </section>

      <ProjectActivityPanel
        v-else-if="activeTab === 'activity'"
        :activity="activity.data.value"
        :error="activity.error.value"
        :loading="activity.loading.value"
        @retry="activity.load(data.id)"
      />


      <form
        v-else-if="data.role === 'owner'"
        class="mt-[22px] grid w-full max-w-lg gap-3 border border-border bg-card p-5"
        @submit.prevent="renameProject"
      >
        <div class="flex items-center gap-2">
          <Pencil :size="15" :stroke-width="1.5" class="text-muted-foreground" />
          <h2 class="text-sm font-medium">Project settings</h2>
        </div>
        <label class="grid gap-2 text-xs text-muted-foreground">
          Project name
          <Input v-model="editName" maxlength="100" />
        </label>
        <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
        <Button class="w-fit" type="submit">Save name</Button>
      </form>
      <section v-else class="mt-[22px] border border-border bg-card px-5 py-8">
        <p class="text-sm font-medium">Read-only project</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Your membership role cannot change project settings.
        </p>
      </section>

      <ServiceDialog
        v-model:open="serviceDialogOpen"
        :error="services.error.value"
        :saving="savingService"
        :service="selectedService"
        @save="saveService"
      />
    </template>
  </div>
</template>
