<script setup lang="ts">
import {
  Activity,
  ArrowLeft,
  Box,
  Boxes,
  Check,
  ChevronLeft,
  ChevronRight,
  Globe2,
  Pencil,
  Plus,
  RefreshCw,
  Rocket,
  Settings2,
  X,
} from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import DeploymentLogsPanel from "@/components/project/DeploymentLogsPanel.vue";
import ProjectActivityPanel from "@/components/project/ProjectActivityPanel.vue";
import ProjectEnvironmentPanel from "@/components/project/ProjectEnvironmentPanel.vue";
import ProjectOverviewPanel from "@/components/project/ProjectOverviewPanel.vue";
import ProjectServiceList from "@/components/project/ProjectServiceList.vue";
import ServiceDomainsPanel from "@/components/project/ServiceDomainsPanel.vue";
import ServiceDialog from "@/components/project/ServiceDialog.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useProject } from "@/composables/useProject";
import { useProjectActivity } from "@/composables/useProjectActivity";
import ProjectDeploymentTimeline from "@/components/project/ProjectDeploymentTimeline.vue";
import { useDeployment } from "@/composables/useDeployment";
import { useDeploymentStream } from "@/composables/useDeploymentStream";
import { useDomains } from "@/composables/useDomains";
import { useService } from "@/composables/useService";
import { useProjectEnvironment } from "@/composables/useProjectEnvironment";
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
const { data, error, load: fetchProject, loading, update } = useProject();
const services = useService();
const deployments = useDeployment();
const domains = useDomains();
const activity = useProjectActivity();
const projectEnvironment = useProjectEnvironment();
const projectTabs = [
  { value: "overview", label: "Overview", icon: Boxes },
  { value: "services", label: "Services", icon: Box },
  { value: "domains", label: "Domains", icon: Globe2 },
  { value: "deployments", label: "Deployments", icon: Rocket },
  { value: "activity", label: "Activity", icon: Activity },
  { value: "environment", label: "Environment", icon: Settings2 },
] as const;
type ProjectTab = (typeof projectTabs)[number]["value"];
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
const serviceError = services.error;
const serviceLoading = services.loading;
const SERVICES_PER_PAGE = 6;
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
const selectedDeployment = computed(() =>
  availableDeployments.value.find((deployment) => deployment.id === selectedDeploymentId.value),
);
const canManage = computed(() => data.value?.role === "owner" || data.value?.role === "editor");
const activeTab = shallowRef<ProjectTab>("overview");
const editName = shallowRef("");
const renamingProject = shallowRef(false);
const serviceDialogOpen = shallowRef(false);
const savingService = shallowRef(false);
let projectLoadGeneration = 0;

watch(
  servicePageCount,
  (count) => {
    if (serviceCurrentPage.value > count) serviceCurrentPage.value = count;
  },
  { immediate: true },
);

function setServiceViewMode(mode: "list" | "catalog") {
  serviceViewMode.value = mode;
  serviceCurrentPage.value = 1;
}

function setActiveTab(value: string | number | undefined) {
  const tab = String(value) as ProjectTab;
  if (projectTabs.some((item) => item.value === tab)) activeTab.value = tab;
}

function goToPreviousServicePage() {
  serviceCurrentPage.value = Math.max(1, serviceCurrentPage.value - 1);
}

function goToNextServicePage() {
  serviceCurrentPage.value = Math.min(servicePageCount.value, serviceCurrentPage.value + 1);
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
  streamLogs.value = [];
  stream.stop();
  logStream.stop();
  if (activeTab.value === "deployments") {
    void stream.connect(deploymentId);
    void logStream.connect(deploymentId);
  }
}

function deploymentServiceName(deployment: DeploymentSummary) {
  return (
    serviceData.value.find((service) => service.id === deployment.service_id)?.name ??
    "Unknown service"
  );
}

function deploymentStatusClass(status: DeploymentSummary["status"]) {
  if (["healthy", "running"].includes(status)) return "text-[var(--status-healthy)]";
  if (status === "failed") return "text-destructive";
  if (["queued", "preparing", "stopping"].includes(status)) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function formatDeploymentTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function selectService(service: ServiceSummary) {
  void router.push({
    name: "ServiceDetail",
    params: { projectId: service.project_id, serviceId: service.id },
  });
}

function load(projectId: string) {
  const generation = ++projectLoadGeneration;
  deployments.clear();
  renamingProject.value = false;
  serviceDialogOpen.value = false;
  void fetchProject(projectId).then(() => {
    if (generation !== projectLoadGeneration) return;
    editName.value = data.value?.name ?? "";
    if (data.value) {
      projectEnvironment.load(data.value.id);
      void loadDeployments(data.value.id, generation);
      void activity.load(data.value.id);
    }
  });
}

async function saveProjectEnvironment(variables: Parameters<typeof projectEnvironment.save>[1]) {
  if (!data.value) return;
  await projectEnvironment.save(data.value.id, variables);
}

async function renameProject() {
  const name = editName.value.trim();
  if (!data.value || !name) return;
  if (name === data.value.name) {
    renamingProject.value = false;
    return;
  }
  const updated = await update({ name });
  if (updated) {
    editName.value = updated.name;
    renamingProject.value = false;
  }
}

function startRenameProject() {
  editName.value = data.value?.name ?? "";
  renamingProject.value = true;
}

function cancelRenameProject() {
  editName.value = data.value?.name ?? "";
  renamingProject.value = false;
}

function createService() {
  serviceDialogOpen.value = true;
}

function openServiceCreator() {
  activeTab.value = "services";
  createService();
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
  if (deployment) selectDeployment(deployment.id);
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
  const service = await services.create(data.value.id, input);
  savingService.value = false;
  if (service) {
    serviceDialogOpen.value = false;
    void router.push({
      name: "ServiceDetail",
      params: { projectId: service.project_id, serviceId: service.id },
    });
  }
}

watch(() => String(route.params.projectId), load, { immediate: true });
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
  <div class="app-page">
    <RouterLink
      class="group inline-flex items-center gap-1.5 text-xs text-muted-foreground transition-colors hover:text-foreground"
      to="/projects"
    >
      <ArrowLeft
        class="size-3.5 transition-transform group-hover:-translate-x-0.5"
        :stroke-width="1.5"
      />
      Back to projects
    </RouterLink>

    <section
      v-if="loading"
      class="mt-6 app-surface px-5 py-6"
      role="status"
      aria-label="Loading project"
    >
      <div class="flex items-center gap-3">
        <Skeleton class="size-11 shrink-0 rounded-[5px]" />
        <div class="grid flex-1 gap-2">
          <Skeleton class="h-5 w-48 max-w-full" />
          <Skeleton class="h-2.5 w-32 max-w-full" />
        </div>
      </div>
      <div class="mt-6 flex gap-4 border-t border-border pt-4">
        <Skeleton v-for="index in 4" :key="index" class="h-3 w-16" />
      </div>
    </section>
    <section
      v-else-if="error"
      class="mt-6 rounded-[10px] border border-destructive/40 bg-card px-5 py-8"
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
      <header class="mt-6 app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
        <div class="flex min-w-0 items-start gap-3">
          <div
            class="grid size-12 shrink-0 place-items-center rounded-[8px] border border-border bg-card text-muted-foreground"
          >
            <Box :size="20" :stroke-width="1.5" />
          </div>
          <div class="min-w-0">
            <p class="ui-label">Project</p>
            <form
              v-if="renamingProject"
              class="mt-2 flex min-w-0 items-center gap-1.5"
              @submit.prevent="renameProject"
            >
              <Input
                v-model="editName"
                class="h-9 min-w-0 max-w-[24rem] text-lg"
                maxlength="100"
                aria-label="Project name"
                required
              />
              <button
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="submit"
                aria-label="Save project name"
                title="Save project name"
              >
                <Check class="size-4" :stroke-width="1.5" />
              </button>
              <button
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="button"
                aria-label="Cancel renaming project"
                title="Cancel"
                @click="cancelRenameProject"
              >
                <X class="size-4" :stroke-width="1.5" />
              </button>
            </form>
            <div v-else class="mt-2 flex min-w-0 items-center gap-1.5">
              <h1 class="m-0 truncate text-3xl leading-none font-normal">
                {{ data.name }}
              </h1>
              <button
                v-if="data.role === 'owner'"
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="button"
                aria-label="Rename project"
                title="Rename project"
                @click="startRenameProject"
              >
                <Pencil class="size-4" :stroke-width="1.5" />
              </button>
            </div>
            <p v-if="renamingProject && error" class="mt-1 text-xs text-destructive" role="alert">
              {{ error }}
            </p>
            <p
              class="mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 font-mono text-[11px] text-muted-foreground"
            >
              <span>{{ data.default_environment.name }} environment</span>
              <span aria-hidden="true">•</span>
              <span class="capitalize">{{ data.role }} access</span>
            </p>
          </div>
        </div>
        <div class="grid min-w-0 gap-3 lg:justify-items-end">
          <Button v-if="canManage" size="sm" @click="openServiceCreator">
            <Plus class="size-4" :stroke-width="1.5" />
            New service
          </Button>
          <Tabs
            :model-value="activeTab"
            class="min-w-0 max-[760px]:w-full"
            @update:model-value="setActiveTab"
          >
            <TabsList
              class="h-9 max-w-full justify-start overflow-x-auto rounded-[4px] max-[760px]:w-full"
              aria-label="Project sections"
            >
              <TabsTrigger
                v-for="tab in projectTabs"
                :key="tab.value"
                :value="tab.value"
                class="min-w-max px-3 text-xs"
              >
                <component :is="tab.icon" class="size-3.5" :stroke-width="1.5" />
                {{ tab.label }}
              </TabsTrigger>
            </TabsList>
          </Tabs>
        </div>
      </header>

      <ProjectOverviewPanel
        v-if="activeTab === 'overview'"
        class="mt-6"
        :activity="activityData"
        :activity-error="activityError"
        :activity-loading="activityLoading"
        :deployment-error="deploymentError"
        :deployments="deploymentData"
        :deployments-loading="deploymentLoading"
        :service-error="serviceError"
        :services="serviceData"
        :services-loading="serviceLoading"
        @retry-activity="activity.load(data.id)"
        @retry-deployments="loadDeployments(data.id)"
        @retry-services="services.load(data.id)"
      />

      <section
        v-else-if="activeTab === 'environment'"
        class="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]"
      >
        <ProjectEnvironmentPanel
          :can-manage="canManage"
          :error="projectEnvironment.error.value"
          :saving="projectEnvironment.saving.value"
          :variables="projectEnvironment.data.value.variables"
          @save="saveProjectEnvironment"
        />
        <aside class="grid content-start gap-4">
          <section class="app-surface grid gap-3 p-5">
            <p class="ui-label">Project scope</p>
            <div class="flex items-center justify-between gap-3 border-b border-border pb-3">
              <span class="text-xs text-muted-foreground">Environment</span>
              <strong class="text-xs font-medium">{{ data.default_environment.name }}</strong>
            </div>
            <div class="flex items-center justify-between gap-3 border-b border-border pb-3">
              <span class="text-xs text-muted-foreground">Role</span>
              <strong class="text-xs font-medium capitalize">{{ data.role }}</strong>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span class="text-xs text-muted-foreground">Shared keys</span>
              <strong class="font-mono text-xs font-medium">{{
                projectEnvironment.data.value.variables.length
              }}</strong>
            </div>
          </section>
          <section class="app-surface-muted p-5">
            <p class="text-xs font-medium">How inheritance works</p>
            <p class="mt-2 text-xs leading-5 text-muted-foreground">
              Each deployment merges project values first. A service-level key with the same name
              wins.
            </p>
          </section>
        </aside>
      </section>

      <section v-else-if="activeTab === 'services'" class="mt-6 grid gap-6">
        <ProjectServiceList
          :can-manage="canManage"
          :error="serviceError"
          :loading="serviceLoading"
          :project-variable-count="projectEnvironment.data.value.variables.length"
          :services="visibleServices"
          :view="serviceViewMode"
          @create="createService"
          @edit="selectService"
          @retry="services.load(data.id)"
          @select="selectService"
          @update-view="setServiceViewMode"
        />
        <nav
          v-if="!serviceLoading && !serviceError && servicePageCount > 1"
          class="app-surface flex items-center justify-between gap-4 px-4 py-3 max-[640px]:items-start max-[640px]:flex-col"
          aria-label="Service pagination"
        >
          <p class="text-xs text-muted-foreground" aria-live="polite">
            Showing {{ firstVisibleService }}–{{ lastVisibleService }} of
            {{ serviceCount }} services
          </p>
          <div class="flex items-center gap-2">
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="serviceCurrentPage === 1"
              aria-label="Previous service page"
              @click="goToPreviousServicePage"
            >
              <ChevronLeft class="size-4" :stroke-width="1.5" />
            </Button>
            <span class="min-w-20 text-center font-mono text-xs text-muted-foreground">
              Page {{ serviceCurrentPage }} of {{ servicePageCount }}
            </span>
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="serviceCurrentPage === servicePageCount"
              aria-label="Next service page"
              @click="goToNextServicePage"
            >
              <ChevronRight class="size-4" :stroke-width="1.5" />
            </Button>
          </div>
        </nav>
      </section>

      <ServiceDomainsPanel
        v-else-if="activeTab === 'domains'"
        class="mt-6"
        :can-manage="canManage"
        :domains="domains.data.value"
        :error="domains.error.value"
        :loading="domains.loading.value"
        :services="serviceData"
        @create="(serviceId, hostname) => domains.create(serviceId, hostname)"
        @remove="(domain: DomainSummary) => domains.remove(domain)"
        @retry="domains.load(serviceData.map((service) => service.id))"
      />

      <section v-else-if="activeTab === 'deployments'" class="mt-6 grid gap-6">
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
        <section
          v-if="availableDeployments.length"
          class="app-surface"
          aria-labelledby="deployment-output-heading"
        >
          <header
            class="app-panel-header flex items-end justify-between gap-4 px-5 py-4 max-[560px]:items-start max-[560px]:flex-col"
          >
            <div>
              <p class="ui-label">Inspect output</p>
              <h2 id="deployment-output-heading" class="mt-2 text-lg font-normal">
                Deployment logs
              </h2>
            </div>
            <p v-if="selectedDeployment" class="font-mono text-[11px] text-muted-foreground">
              {{ deploymentServiceName(selectedDeployment) }} · g{{ selectedDeployment.generation }}
            </p>
          </header>
          <div class="divide-y divide-border" aria-label="Select deployment logs">
            <button
              v-for="deployment in availableDeployments"
              :key="deployment.id"
              class="grid w-full gap-2 px-5 py-3.5 text-left transition-colors hover:bg-muted/60 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
              :class="selectedDeploymentId === deployment.id ? 'bg-muted/70' : ''"
              type="button"
              :aria-pressed="selectedDeploymentId === deployment.id"
              @click="selectDeployment(deployment.id)"
            >
              <span class="grid min-w-0 gap-1">
                <span class="truncate text-sm font-medium">{{
                  deploymentServiceName(deployment)
                }}</span>
                <span class="truncate font-mono text-[11px] text-muted-foreground">
                  Generation g{{ deployment.generation }} ·
                  {{ formatDeploymentTime(deployment.created_at) }}
                </span>
              </span>
              <span
                class="justify-self-start text-xs capitalize sm:justify-self-end"
                :class="deploymentStatusClass(deployment.status)"
              >
                {{ deployment.status }}
              </span>
            </button>
          </div>
        </section>
        <DeploymentLogsPanel
          v-if="selectedDeployment"
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

      <ServiceDialog
        v-model:open="serviceDialogOpen"
        :error="serviceError"
        :inherited-variables="projectEnvironment.data.value.variables"
        :saving="savingService"
        @save="saveService"
      />
    </template>
  </div>
</template>
