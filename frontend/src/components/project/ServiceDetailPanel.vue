<script setup lang="ts">
import {
  Box,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  CircleDotDashed,
  GitBranch,
  Pencil,
  Rocket,
  RotateCcw,
  Settings2,
  Square,
} from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import DeploymentApprovalPanel from "@/components/project/DeploymentApprovalPanel.vue";
import DeploymentLogsPanel from "@/components/project/DeploymentLogsPanel.vue";
import DeploymentSupplyChainPanel from "@/components/project/DeploymentSupplyChainPanel.vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type {
  DeploymentLog,
  DeploymentState,
  DeploymentSummary,
  ServiceSummary,
} from "@/lib/types";

const props = defineProps<{
  service: ServiceSummary;
  deployments: DeploymentSummary[];
  logs: DeploymentLog[];
  connected: boolean;
  streamError: string | null;
  submitting: boolean;
  canManage: boolean;
  canApprove?: boolean;
  hideConfig?: boolean;
  hideHeader?: boolean;
  selectedDeploymentId: string | null;
}>();

const emit = defineEmits<{
  edit: [service: ServiceSummary];
  deploy: [serviceId: string];
  stop: [serviceId: string];
  cancel: [deploymentId: string];
  approve: [deploymentId: string];
  rollback: [deploymentId: string];
  selectDeployment: [deploymentId: string];
}>();

const activeTab = shallowRef<"config" | "deployment" | "logs">(
  props.hideConfig ? "deployment" : "config",
);
const detailTabs = computed(() =>
  props.hideConfig
    ? [
        { value: "deployment" as const, label: "Deployments" },
        { value: "logs" as const, label: "Logs" },
      ]
    : [
        { value: "config" as const, label: "Configuration" },
        { value: "deployment" as const, label: "Deployments" },
        { value: "logs" as const, label: "Logs" },
      ],
);
const serviceDeployments = computed(() =>
  props.deployments.filter((deployment) => deployment.service_id === props.service.id),
);
const DEPLOYMENTS_PER_PAGE = 6;
const deploymentCurrentPage = shallowRef(1);
const deploymentCount = computed(() => serviceDeployments.value.length);
const deploymentPageCount = computed(() =>
  Math.max(1, Math.ceil(deploymentCount.value / DEPLOYMENTS_PER_PAGE)),
);
const visibleDeployments = computed(() => {
  const start = (deploymentCurrentPage.value - 1) * DEPLOYMENTS_PER_PAGE;
  return serviceDeployments.value.slice(start, start + DEPLOYMENTS_PER_PAGE);
});
const firstVisibleDeployment = computed(() =>
  deploymentCount.value === 0 ? 0 : (deploymentCurrentPage.value - 1) * DEPLOYMENTS_PER_PAGE + 1,
);
const lastVisibleDeployment = computed(() =>
  Math.min(deploymentCurrentPage.value * DEPLOYMENTS_PER_PAGE, deploymentCount.value),
);
const latestDeployment = computed(() => serviceDeployments.value[0] ?? null);
const rollbackTarget = computed(
  () =>
    serviceDeployments.value.find(
      (deployment) =>
        deployment.id !== serviceDeployments.value[0]?.id &&
        ["healthy", "superseded", "stopped"].includes(deployment.status),
    ) ?? null,
);
const needsConfiguration = computed(() => props.service.source_config?.setup_required === true);
const canStop = computed(
  () =>
    !needsConfiguration.value &&
    ["queued", "preparing", "running", "healthy"].includes(latestDeployment.value?.status ?? ""),
);
const canRollback = computed(() =>
  Boolean(
    rollbackTarget.value &&
    rollbackTarget.value.id !== latestDeployment.value?.id &&
    props.canManage &&
    !props.submitting,
  ),
);
const canCancel = computed(() =>
  ["queued", "preparing"].includes(latestDeployment.value?.status ?? ""),
);
const sourceLabel = computed(() => {
  if (needsConfiguration.value) return "Setup required";
  if (props.service.source_config?.source === "application") {
    const repository = props.service.source_config.repository ?? "repository";
    const branch = props.service.source_config.branch
      ? `@${props.service.source_config.branch}`
      : "";
    return `${props.service.source_config.builder ?? "application"} · ${repository}${branch}`;
  }
  if (props.service.source_config?.source === "template") {
    return `Template · ${props.service.source_config.template ?? "runtime"}`;
  }
  if (props.service.source_config?.repository) {
    const path = props.service.source_config.dockerfile_path ?? "docker-compose.yml";
    return `${props.service.source_config.repository} · ${path}`;
  }
  return "Inline Compose";
});
const deployLabel = computed(() =>
  props.service.source_config?.source === "application"
    ? latestDeployment.value
      ? "Request rebuild"
      : "Request production build"
    : latestDeployment.value
      ? "Request deployment"
      : "Request production deployment",
);
const connectionLabel = computed(() => {
  if (!latestDeployment.value) return "No live deployment";
  return props.connected ? "Live updates" : "Updates reconnecting";
});

watch(
  () => [props.service.id, props.hideConfig] as const,
  () => {
    activeTab.value = props.hideConfig ? "deployment" : "config";
    deploymentCurrentPage.value = 1;
  },
);

watch(
  deploymentPageCount,
  (count) => {
    if (deploymentCurrentPage.value > count) deploymentCurrentPage.value = count;
  },
  { immediate: true },
);

watch(
  [serviceDeployments, () => props.selectedDeploymentId],
  ([deployments, deploymentId]) => {
    const selectedIndex = deployments.findIndex((deployment) => deployment.id === deploymentId);
    if (selectedIndex >= 0) {
      deploymentCurrentPage.value = Math.floor(selectedIndex / DEPLOYMENTS_PER_PAGE) + 1;
    }
  },
  { immediate: true },
);

function statusBadgeClass(status: DeploymentState) {
  if (status === "healthy") {
    return "border-[var(--status-healthy)]/40 bg-[var(--status-healthy)]/10 text-[var(--status-healthy)]";
  }
  if (status === "failed") return "border-destructive/40 bg-destructive/10 text-destructive";
  if (
    status === "running" ||
    status === "preparing" ||
    status === "queued" ||
    status === "stopping"
  ) {
    return "border-[var(--status-live)]/40 bg-[var(--status-live)]/10 text-[var(--status-live)]";
  }
  return "border-border bg-muted/50 text-muted-foreground";
}

function statusDotState(status: DeploymentState) {
  if (status === "healthy") return "healthy";
  if (status === "failed") return "failed";
  if (
    status === "running" ||
    status === "preparing" ||
    status === "queued" ||
    status === "stopping"
  ) {
    return "live";
  }
  return "inactive";
}

function formatTimestamp(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatRetry(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(date);
}

function selectDeployment(deployment: DeploymentSummary) {
  activeTab.value = "logs";
  emit("selectDeployment", deployment.id);
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
</script>

<template>
  <section
    class="overflow-hidden rounded-[10px] border border-border bg-card"
    aria-labelledby="service-detail-title"
  >
    <header
      class="app-panel-header flex items-start justify-between gap-4 px-5 py-4 max-[560px]:flex-col"
    >
      <div v-if="!hideHeader" class="flex min-w-0 items-start gap-3">
        <span
          class="grid size-9 shrink-0 place-items-center rounded-[6px] border border-border bg-card text-muted-foreground"
        >
          <GitBranch
            v-if="service.source_config?.source === 'application'"
            class="size-4"
            :stroke-width="1.5"
          />
          <Box v-else class="size-4" :stroke-width="1.5" />
        </span>
        <div class="min-w-0">
          <p class="ui-label">Service detail</p>
          <h2 id="service-detail-title" class="mt-2 truncate text-xl font-normal">
            {{ service.name }}
          </h2>
          <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground">{{ sourceLabel }}</p>
        </div>
      </div>
      <div v-else class="min-w-0">
        <p class="ui-label">Operations</p>
        <h2 id="service-detail-title" class="mt-1.5 text-base font-medium">Deployments and logs</h2>
        <p class="mt-1 truncate text-xs text-muted-foreground">
          Track revisions and inspect live output.
        </p>
      </div>
      <div class="flex flex-wrap gap-2 max-[560px]:w-full">
        <Button
          v-if="canManage && !hideConfig"
          size="sm"
          class="max-[560px]:flex-1"
          variant="outline"
          @click="emit('edit', service)"
        >
          <Pencil data-icon="inline-start" :stroke-width="1.5" />
          {{ needsConfiguration ? "Set up service" : "Configure" }}
        </Button>
        <Button
          v-if="canManage"
          size="sm"
          class="max-[560px]:flex-1"
          :disabled="submitting || needsConfiguration"
          @click="emit('deploy', service.id)"
        >
          <Rocket data-icon="inline-start" :stroke-width="1.5" />
          {{ deployLabel }}
        </Button>
        <Button
          v-if="canManage && canStop"
          size="sm"
          class="max-[560px]:flex-1"
          variant="outline"
          :disabled="submitting"
          @click="
            canCancel && latestDeployment
              ? emit('cancel', latestDeployment.id)
              : emit('stop', service.id)
          "
        >
          <Square data-icon="inline-start" :stroke-width="1.5" />
          {{ canCancel ? "Cancel" : "Stop" }}
        </Button>
      </div>
    </header>

    <div
      class="flex items-center justify-between gap-4 border-b border-border px-5 py-3 max-[560px]:items-start max-[560px]:flex-col"
    >
      <Tabs v-model="activeTab" class="max-[560px]:w-full">
        <TabsList class="h-8 rounded-[4px] max-[560px]:w-full" aria-label="Service detail sections">
          <TabsTrigger
            v-for="tab in detailTabs"
            :key="tab.value"
            :value="tab.value"
            class="max-[560px]:flex-1"
            @click="activeTab = tab.value"
          >
            {{ tab.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <Badge variant="outline" class="gap-1.5 rounded-[4px] font-normal text-muted-foreground">
        <CircleDotDashed class="size-3" :stroke-width="1.5" />
        {{ connectionLabel }}
      </Badge>
    </div>

    <div v-if="activeTab === 'config'" class="grid gap-6 p-5">
      <Alert v-if="needsConfiguration" class="border-[var(--status-live)]/40">
        <CircleAlert :stroke-width="1.5" />
        <AlertTitle>Setup required</AlertTitle>
        <AlertDescription>Complete the service configuration before deploying.</AlertDescription>
      </Alert>

      <div
        class="grid divide-y divide-border border-y border-border sm:grid-cols-3 sm:divide-x sm:divide-y-0"
      >
        <div class="px-0 py-3 sm:px-4 sm:first:pl-0">
          <p class="ui-label">Desired state</p>
          <p class="mt-2 text-sm capitalize">{{ service.desired_state }}</p>
        </div>
        <div class="px-0 py-3 sm:px-4 sm:py-3">
          <p class="ui-label">Generation</p>
          <p class="mt-2 font-mono text-sm">g{{ service.desired_generation }}</p>
        </div>
        <div class="px-0 py-3 sm:px-4 sm:pr-0">
          <p class="ui-label">Internal port</p>
          <p class="mt-2 font-mono text-sm">{{ service.internal_port ?? "Not set" }}</p>
        </div>
      </div>

      <div class="grid gap-3">
        <div class="flex items-center justify-between gap-3">
          <div>
            <p class="text-sm font-medium">Environment overrides</p>
            <p class="mt-1 text-xs text-muted-foreground">Service values take precedence.</p>
          </div>
          <Button v-if="canManage" size="sm" variant="outline" @click="emit('edit', service)">
            <Settings2 data-icon="inline-start" :stroke-width="1.5" />
            Edit
          </Button>
        </div>
        <div v-if="service.variables.length" class="divide-y divide-border border border-border">
          <div
            v-for="variable in service.variables"
            :key="variable.key"
            class="flex items-center justify-between gap-3 px-3 py-2.5"
          >
            <code class="font-mono text-xs">{{ variable.key }}</code>
            <span class="text-xs text-muted-foreground">
              {{ variable.is_set ? (variable.is_secret ? "********" : variable.value) : "Not set" }}
            </span>
          </div>
        </div>
        <p v-else class="text-xs text-muted-foreground">No service-specific environment keys.</p>
      </div>
    </div>

    <div v-else-if="activeTab === 'deployment'" class="grid gap-6 p-5">
      <div class="flex flex-wrap items-center justify-between gap-3 border-b border-border pb-5">
        <div>
          <p class="ui-label">Deployment state</p>
          <div class="mt-2 flex items-center gap-2">
            <Badge
              v-if="latestDeployment"
              variant="outline"
              :class="statusBadgeClass(latestDeployment.status)"
            >
              <Check v-if="latestDeployment.status === 'healthy'" :stroke-width="1.5" />
              {{ latestDeployment.status }}
            </Badge>
            <span v-else class="text-sm text-muted-foreground">No deployment yet</span>
            <span v-if="latestDeployment" class="font-mono text-[11px] text-muted-foreground">
              g{{ latestDeployment.generation }}
            </span>
          </div>
        </div>
        <div class="flex flex-wrap gap-2">
          <Button
            v-if="canManage && canRollback"
            size="sm"
            variant="outline"
            :disabled="submitting"
            @click="rollbackTarget && emit('rollback', rollbackTarget.id)"
          >
            <RotateCcw data-icon="inline-start" :stroke-width="1.5" />
            Rollback
          </Button>
        </div>
      </div>

      <Alert v-if="latestDeployment?.failure_reason" variant="destructive">
        <CircleAlert :stroke-width="1.5" />
        <AlertTitle>Latest deployment failed</AlertTitle>
        <AlertDescription>{{ latestDeployment.failure_reason }}</AlertDescription>
      </Alert>
      <Alert v-else-if="latestDeployment?.retry_after" class="border-[var(--status-live)]/40">
        <CircleDotDashed :stroke-width="1.5" />
        <AlertTitle>Retry scheduled</AlertTitle>
        <AlertDescription>
          Runtime start attempt {{ latestDeployment.attempt_count }} did not complete. Ignitify will
          retry after {{ formatRetry(latestDeployment.retry_after) }}.
        </AlertDescription>
      </Alert>

      <DeploymentSupplyChainPanel :report="latestDeployment?.supply_chain_report ?? null" />
      <DeploymentApprovalPanel
        v-if="latestDeployment?.approval"
        :approval="latestDeployment.approval"
        :identity="latestDeployment.source_identity"
        :can-approve="canApprove"
        :submitting="submitting"
        @approve="emit('approve', latestDeployment.id)"
      />

      <div v-if="serviceDeployments.length" class="divide-y divide-border border-y border-border">
        <div
          v-for="deployment in visibleDeployments"
          :key="deployment.id"
          class="flex items-center gap-3 px-2 py-3 transition-colors max-[560px]:items-start max-[560px]:flex-col"
          :class="selectedDeploymentId === deployment.id ? 'bg-muted' : 'hover:bg-muted/40'"
        >
          <Button
            variant="ghost"
            class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-1 rounded-[4px] text-left outline-none focus-visible:ring-2 focus-visible:ring-ring/50 sm:grid-cols-[auto_minmax(0,1fr)_auto]"
            type="button"
            :aria-current="selectedDeploymentId === deployment.id ? 'true' : undefined"
            @click="selectDeployment(deployment)"
          >
            <span
              class="status-dot"
              :data-status="statusDotState(deployment.status)"
              aria-hidden="true"
            />
            <span class="grid min-w-0 gap-1">
              <span class="text-sm font-medium">Generation {{ deployment.generation }}</span>
              <span class="truncate font-mono text-[11px] text-muted-foreground">
                {{ formatTimestamp(deployment.created_at) }}
              </span>
            </span>
            <Badge
              variant="outline"
              class="col-start-2 justify-self-start sm:col-auto sm:justify-self-auto"
              :class="statusBadgeClass(deployment.status)"
            >
              {{ deployment.status }}
            </Badge>
          </Button>
          <Button
            v-if="
              canManage &&
              ['healthy', 'superseded', 'stopped'].includes(deployment.status) &&
              deployment.id !== latestDeployment?.id
            "
            size="sm"
            variant="outline"
            :disabled="submitting"
            @click="emit('rollback', deployment.id)"
          >
            <RotateCcw data-icon="inline-start" :stroke-width="1.5" />
            Rollback
          </Button>
        </div>
      </div>
      <p v-else class="text-sm text-muted-foreground">
        Deploy this service to create its first revision.
      </p>
      <nav
        v-if="deploymentPageCount > 1"
        class="flex items-center justify-between gap-4 border-t border-border pt-4 max-[560px]:items-start max-[560px]:flex-col"
        aria-label="Deployment history pagination"
      >
        <p class="text-xs text-muted-foreground" aria-live="polite">
          Showing {{ firstVisibleDeployment }}–{{ lastVisibleDeployment }} of
          {{ deploymentCount }} deployments
        </p>
        <div class="flex items-center gap-2">
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

    <div v-else class="p-5">
      <DeploymentLogsPanel
        embedded
        :connected="connected"
        :logs="logs"
        :stream-error="streamError"
      />
    </div>
  </section>
</template>
