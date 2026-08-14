<script setup lang="ts">
import {
  Activity,
  ArrowLeft,
  AlertTriangle,
  Box,
  Boxes,
  Check,
  ChevronLeft,
  ChevronRight,
  Copy,
  Pencil,
  Plus,
  Rocket,
  Settings2,
  Trash2,
  X,
} from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { RouterLink, useRoute, useRouter } from "vue-router";
import { toast } from "vue-sonner";
import DeploymentLogsPanel from "@/components/project/DeploymentLogsPanel.vue";
import ProjectActivityPanel from "@/components/project/ProjectActivityPanel.vue";
import ProjectEnvironmentPanel from "@/components/project/ProjectEnvironmentPanel.vue";
import ProjectOverviewPanel from "@/components/project/ProjectOverviewPanel.vue";
import ProjectServiceList from "@/components/project/ProjectServiceList.vue";
import ServiceDialog from "@/components/project/ServiceDialog.vue";
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
import { useProject } from "@/composables/useProject";
import ProjectDeploymentTimeline from "@/components/project/ProjectDeploymentTimeline.vue";
import { useProjectDeploymentActivity } from "@/composables/useProjectDeploymentActivity";
import { useProjectEnvironment } from "@/composables/useProjectEnvironment";
import type { ServiceInput } from "@/lib/types";

const route = useRoute();
const router = useRouter();
const { data, error, load: fetchProject, loading, remove: removeProject, update } = useProject();
const projectEnvironment = useProjectEnvironment();
const projectTabs = [
  { value: "overview", label: "Overview", icon: Boxes },
  { value: "services", label: "Services", icon: Box },
  { value: "deployments", label: "Deployments", icon: Rocket },
  { value: "activity", label: "Activity", icon: Activity },
  { value: "environment", label: "Environment", icon: Settings2 },
] as const;
type ProjectTab = (typeof projectTabs)[number]["value"];
const canManage = computed(() => data.value?.role === "owner" || data.value?.role === "editor");
const projectVariableCount = computed(
  () => projectEnvironment.data.value.variables.filter((variable) => !variable.is_secret).length,
);
const projectSecretCount = computed(
  () => projectEnvironment.data.value.variables.filter((variable) => variable.is_secret).length,
);
const canDeleteProject = computed(() => data.value?.role === "owner");
const activeTab = shallowRef<ProjectTab>("overview");
const {
  activity,
  activityData,
  activityError,
  activityLoading,
  approveDeployment,
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
  logStream,
  rollbackDeployment,
  selectedDeployment,
  selectedDeploymentId,
  selectDeploymentAndRevealLogs,
  serviceCount,
  serviceCurrentPage,
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
  submitDeployment,
  visibleDeployments,
  visibleServices,
} = useProjectDeploymentActivity(activeTab);
const editName = shallowRef("");
const renamingProject = shallowRef(false);
const serviceDialogOpen = shallowRef(false);
const savingService = shallowRef(false);
const deleteProjectOpen = shallowRef(false);
const deleteProjectConfirmName = shallowRef("");
const deletingProject = shallowRef(false);
const copiedProjectName = shallowRef(false);
let copyProjectNameTimer: number | undefined;

function setActiveTab(value: string | number | undefined) {
  const tab = String(value) as ProjectTab;
  if (projectTabs.some((item) => item.value === tab)) activeTab.value = tab;
}

async function load(projectId: string) {
  renamingProject.value = false;
  serviceDialogOpen.value = false;
  await fetchProject(projectId);
  if (!data.value) {
    toast.error("Project unavailable", {
      description: error.value ?? "Could not load project.",
      action: { label: "Retry", onClick: () => void load(projectId) },
    });
    return;
  }
  editName.value = data.value.name;
  void projectEnvironment.load(data.value.id);
  void loadProjectWorkloads(data.value.id);
}

async function saveProjectEnvironment(variables: Parameters<typeof projectEnvironment.save>[1]) {
  if (!data.value) return;
  const saved = await projectEnvironment.save(data.value.id, variables);
  if (saved) {
    toast.success("Project environment saved");
    return;
  }
  toast.error("Could not save project environment", {
    description: projectEnvironment.error.value ?? "Try again in a moment.",
  });
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
    toast.success("Project renamed", { description: updated.name });
    return;
  }
  toast.error("Could not rename project", { description: error.value ?? "Try again in a moment." });
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

function requestDeleteProject() {
  deleteProjectOpen.value = true;
  deleteProjectConfirmName.value = "";
  copiedProjectName.value = false;
}

function cancelDeleteProject() {
  deleteProjectOpen.value = false;
  deleteProjectConfirmName.value = "";
  copiedProjectName.value = false;
  error.value = null;
}

async function copyProjectName() {
  const name = data.value?.name;
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
    toast.error("Could not copy project name");
    return;
  }

  deleteProjectConfirmName.value = name;
  copiedProjectName.value = true;
  toast.success("Project name copied");
  if (copyProjectNameTimer !== undefined) window.clearTimeout(copyProjectNameTimer);
  copyProjectNameTimer = window.setTimeout(() => {
    copiedProjectName.value = false;
    copyProjectNameTimer = undefined;
  }, 1_600);
}

async function deleteProject() {
  const current = data.value;
  if (!current || deleteProjectConfirmName.value !== current.name) return;
  deletingProject.value = true;
  const removed = await removeProject(deleteProjectConfirmName.value);
  deletingProject.value = false;
  if (!removed) {
    toast.error("Could not delete project", {
      description: error.value ?? "Try again in a moment.",
    });
    return;
  }
  deleteProjectOpen.value = false;
  toast.success("Project deleted", { description: current.name });
  await router.push({ name: "Projects" });
}

async function saveService(input: ServiceInput) {
  if (!data.value) return;
  savingService.value = true;
  const service = await services.create(data.value.id, input);
  savingService.value = false;
  if (service) {
    serviceDialogOpen.value = false;
    toast.success("Service created", { description: service.name });
    void router.push({
      name: "ServiceDetail",
      params: { projectId: service.project_id, serviceId: service.id },
    });
    return;
  }
  toast.error("Could not create service", {
    description: serviceError.value ?? "Try again in a moment.",
  });
}

watch(() => String(route.params.projectId), load, { immediate: true });
onUnmounted(() => {
  if (copyProjectNameTimer !== undefined) window.clearTimeout(copyProjectNameTimer);
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
              <Button
                variant="ghost"
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="submit"
                aria-label="Save project name"
                title="Save project name"
              >
                <Check class="size-4" :stroke-width="1.5" />
              </Button>
              <Button
                variant="ghost"
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="button"
                aria-label="Cancel renaming project"
                title="Cancel"
                @click="cancelRenameProject"
              >
                <X class="size-4" :stroke-width="1.5" />
              </Button>
            </form>
            <div v-else class="mt-2 flex min-w-0 items-center gap-1.5">
              <h1 class="m-0 truncate text-3xl leading-none font-normal">
                {{ data.name }}
              </h1>
              <Button
                variant="ghost"
                v-if="data.role === 'owner'"
                class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
                type="button"
                aria-label="Rename project"
                title="Rename project"
                @click="startRenameProject"
              >
                <Pencil class="size-4" :stroke-width="1.5" />
              </Button>
            </div>
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
          <div class="flex flex-wrap items-center gap-2 lg:justify-end">
            <Button v-if="canManage" size="sm" @click="openServiceCreator">
              <Plus class="size-4" :stroke-width="1.5" />
              New service
            </Button>
            <Button
              v-if="canDeleteProject"
              size="sm"
              variant="destructive"
              @click="requestDeleteProject"
            >
              <Trash2 class="size-4" :stroke-width="1.5" />
              Delete project
            </Button>
          </div>
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
        @retry-deployments="loadProjectWorkloads(data.id)"
        @retry-services="services.load(data.id)"
      />

      <section
        v-else-if="activeTab === 'environment'"
        class="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]"
      >
        <ProjectEnvironmentPanel
          :can-manage="canManage"
          :error="projectEnvironment.error.value"
          :loading="projectEnvironment.loading.value"
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
            <div class="flex items-center justify-between gap-3 border-b border-border pb-3">
              <span class="text-xs text-muted-foreground">Variables</span>
              <strong class="font-mono text-xs font-medium">{{ projectVariableCount }}</strong>
            </div>
            <div class="flex items-center justify-between gap-3">
              <span class="text-xs text-muted-foreground">Secrets</span>
              <strong class="font-mono text-xs font-medium">{{ projectSecretCount }}</strong>
            </div>
          </section>
          <section class="app-surface-muted p-5">
            <p class="text-xs font-medium">How inheritance works</p>
            <p class="mt-2 text-xs leading-5 text-muted-foreground">
              Project values are inherited by every service. A service-level key with the same name
              wins, while secret values stay masked.
            </p>
          </section>
        </aside>
      </section>

      <section v-else-if="activeTab === 'services'" class="mt-6 grid gap-4">
        <ProjectServiceList
          :can-manage="canManage"
          :error="serviceError"
          :loading="serviceLoading"
          :project-variable-count="projectEnvironment.data.value.variables.length"
          :services="visibleServices"
          :view="serviceViewMode"
          @create="createService"
          @retry="services.load(data.id)"
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

      <section
        v-else-if="activeTab === 'deployments'"
        class="mt-6 grid min-w-0 items-start gap-4"
        :class="
          selectedDeployment ? 'lg:grid-cols-[minmax(20rem,0.85fr)_minmax(0,1.4fr)]' : undefined
        "
      >
        <div class="order-2 grid min-w-0 gap-3 lg:order-1">
          <ProjectDeploymentTimeline
            :can-approve="data.role === 'owner'"
            :deployments="visibleDeployments"
            :error="deploymentError"
            :loading="deploymentLoading"
            :services="serviceData"
            :submitting="deploymentSubmitting"
            :selected-deployment-id="selectedDeploymentId"
            @deploy="submitDeployment"
            @approve="approveDeployment"
            @stop="stopDeployment"
            @retry="loadProjectWorkloads(data.id)"
            @rollback="rollbackDeployment"
            @select="selectDeploymentAndRevealLogs"
          />
          <nav
            v-if="!deploymentLoading && deploymentPageCount > 1"
            class="flex items-center justify-between gap-3 border-t border-border px-1 pt-3 max-[560px]:items-start max-[560px]:flex-col"
            aria-label="Deployment history pagination"
          >
            <p class="text-xs text-muted-foreground" aria-live="polite">
              Showing {{ firstVisibleDeployment }}–{{ lastVisibleDeployment }} of
              {{ deploymentCount }} deployments
            </p>
            <div class="flex shrink-0 items-center gap-2">
              <Button
                size="icon-sm"
                variant="outline"
                :disabled="deploymentCurrentPage === 1"
                aria-label="Previous deployment page"
                @click="goToPreviousDeploymentPage"
              >
                <ChevronLeft class="size-4" :stroke-width="1.5" />
              </Button>
              <span class="min-w-20 text-center font-mono text-xs text-muted-foreground">
                Page {{ deploymentCurrentPage }} of {{ deploymentPageCount }}
              </span>
              <Button
                size="icon-sm"
                variant="outline"
                :disabled="deploymentCurrentPage === deploymentPageCount"
                aria-label="Next deployment page"
                @click="goToNextDeploymentPage"
              >
                <ChevronRight class="size-4" :stroke-width="1.5" />
              </Button>
            </div>
          </nav>
        </div>
        <div
          v-if="selectedDeployment"
          :ref="setDeploymentLogsAnchor"
          class="order-1 min-w-0 scroll-mt-4 lg:sticky lg:top-4 lg:order-2"
        >
          <DeploymentLogsPanel
            :connected="stream.connected.value && logStream.connected.value"
            :logs="streamLogs"
            :stream-error="stream.error.value ?? logStream.error.value"
          />
        </div>
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

      <AlertDialogRoot v-model:open="deleteProjectOpen">
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
                <AlertDialogTitle class="text-base font-medium">Delete project?</AlertDialogTitle>
                <AlertDialogDescription class="mt-2 text-sm leading-5">
                  This permanently removes
                  <span class="inline-flex max-w-full items-center gap-1 align-bottom">
                    <span class="max-w-[18rem] truncate font-medium text-foreground">{{
                      data.name
                    }}</span>
                    <Button
                      variant="ghost"
                      class="grid size-5 shrink-0 place-items-center rounded-[3px] text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                      type="button"
                      :aria-label="
                        copiedProjectName
                          ? 'Project name copied and filled'
                          : 'Copy and fill project name'
                      "
                      :title="
                        copiedProjectName ? 'Copied and filled' : 'Copy and fill project name'
                      "
                      @click="copyProjectName"
                    >
                      <Check v-if="copiedProjectName" class="size-3" :stroke-width="1.75" />
                      <Copy v-else class="size-3" :stroke-width="1.5" />
                    </Button>
                  </span>
                  and its services, deployments, logs, domains, and shared variables.
                </AlertDialogDescription>
              </div>
            </div>
            <Label class="grid gap-2 text-xs text-muted-foreground" for="delete-project-name">
              Type the project name to confirm
              <Input
                id="delete-project-name"
                v-model="deleteProjectConfirmName"
                :placeholder="data.name"
                autocomplete="off"
                :disabled="deletingProject"
              />
            </Label>
            <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
              <AlertDialogCancel as-child>
                <Button
                  class="w-full sm:w-auto"
                  variant="outline"
                  :disabled="deletingProject"
                  @click="cancelDeleteProject"
                >
                  Cancel
                </Button>
              </AlertDialogCancel>
              <AlertDialogAction as-child>
                <Button
                  class="w-full sm:w-auto"
                  variant="destructive"
                  :disabled="deletingProject || deleteProjectConfirmName !== data.name"
                  @click.prevent="deleteProject"
                >
                  <Trash2 class="size-4" :stroke-width="1.5" />
                  {{ deletingProject ? "Deleting..." : "Delete project" }}
                </Button>
              </AlertDialogAction>
            </div>
          </AlertDialogContent>
        </AlertDialogPortal>
      </AlertDialogRoot>
    </template>
  </div>
</template>
