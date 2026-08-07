<script setup lang="ts">
import {
  Box,
  Check,
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
import DeploymentLogsPanel from "@/components/project/DeploymentLogsPanel.vue";
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
  hideConfig?: boolean;
  selectedDeploymentId: string | null;
}>();

const emit = defineEmits<{
  edit: [service: ServiceSummary];
  deploy: [serviceId: string];
  stop: [serviceId: string];
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
const latestDeployment = computed(() => serviceDeployments.value[0] ?? null);
const selectedDeployment = computed(
  () =>
    serviceDeployments.value.find((deployment) => deployment.id === props.selectedDeploymentId) ??
    null,
);
const rollbackTarget = computed(
  () => serviceDeployments.value.find((deployment) => deployment.status === "healthy") ?? null,
);
const needsConfiguration = computed(() => props.service.source_config?.setup_required === true);
const canStop = computed(
  () =>
    !needsConfiguration.value &&
    props.service.desired_state !== "stopped" &&
    !["stopping", "stopped"].includes(latestDeployment.value?.status ?? ""),
);
const canRollback = computed(() =>
  Boolean(
    rollbackTarget.value &&
    rollbackTarget.value.id !== latestDeployment.value?.id &&
    props.canManage &&
    !props.submitting,
  ),
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
  props.service.source_config?.source === "application" ? "Build & deploy" : "Deploy",
);
const connectionLabel = computed(() => {
  if (!latestDeployment.value) return "No live deployment";
  return props.connected ? "Live updates" : "Updates reconnecting";
});

watch(
  () => [props.service.id, props.hideConfig] as const,
  () => {
    activeTab.value = props.hideConfig ? "deployment" : "config";
  },
);

function statusVariant(
  status: DeploymentState,
): "default" | "secondary" | "destructive" | "outline" {
  if (status === "healthy") return "default";
  if (status === "failed") return "destructive";
  if (status === "running" || status === "preparing" || status === "queued") return "secondary";
  return "outline";
}

function statusClass(status: DeploymentState) {
  if (status === "healthy") return "text-[var(--status-healthy)]";
  if (status === "failed") return "text-destructive";
  if (status === "running" || status === "preparing" || status === "queued") {
    return "text-[var(--status-live)]";
  }
  return "text-muted-foreground";
}

function statusDotClass(status: DeploymentState) {
  if (status === "healthy") return "bg-[var(--status-healthy)]";
  if (status === "failed") return "bg-destructive";
  if (status === "running" || status === "preparing" || status === "queued") {
    return "bg-[var(--status-live)]";
  }
  return "bg-muted-foreground";
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

function selectDeployment(deployment: DeploymentSummary) {
  activeTab.value = "logs";
  emit("selectDeployment", deployment.id);
}
</script>

<template>
  <section class="border border-border bg-card" aria-labelledby="service-detail-title">
    <header
      class="flex items-start justify-between gap-4 border-b border-border px-5 py-5 max-[560px]:flex-col"
    >
      <div class="flex min-w-0 items-start gap-3">
        <span
          class="grid size-9 shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
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
      <div class="flex flex-wrap gap-2">
        <Button v-if="canManage" size="sm" variant="outline" @click="emit('edit', service)">
          <Pencil data-icon="inline-start" :stroke-width="1.5" />
          {{ needsConfiguration ? "Set up service" : "Configure" }}
        </Button>
        <Button
          v-if="canManage"
          size="sm"
          :disabled="submitting || needsConfiguration"
          @click="emit('deploy', service.id)"
        >
          <Rocket data-icon="inline-start" :stroke-width="1.5" />
          {{ deployLabel }}
        </Button>
        <Button
          v-if="canManage && canStop"
          size="sm"
          variant="outline"
          :disabled="submitting"
          @click="emit('stop', service.id)"
        >
          <Square data-icon="inline-start" :stroke-width="1.5" />
          Stop
        </Button>
      </div>
    </header>

    <div
      class="flex items-center justify-between gap-4 border-b border-border px-5 py-3 max-[560px]:items-start max-[560px]:flex-col"
    >
      <Tabs v-model="activeTab">
        <TabsList aria-label="Service detail sections">
          <TabsTrigger
            v-for="tab in detailTabs"
            :key="tab.value"
            :value="tab.value"
            @click="activeTab = tab.value"
          >
            {{ tab.label }}
          </TabsTrigger>
        </TabsList>
      </Tabs>
      <Badge variant="outline" class="gap-1.5 font-normal text-muted-foreground">
        <CircleDotDashed class="size-3" :stroke-width="1.5" />
        {{ connectionLabel }}
      </Badge>
    </div>

    <div v-if="activeTab === 'config'" class="grid gap-5 p-5">
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

    <div v-else-if="activeTab === 'deployment'" class="grid gap-5 p-5">
      <div class="flex flex-wrap items-center justify-between gap-3">
        <div>
          <p class="ui-label">Deployment state</p>
          <div class="mt-2 flex items-center gap-2">
            <Badge
              v-if="latestDeployment"
              :variant="statusVariant(latestDeployment.status)"
              :class="statusClass(latestDeployment.status)"
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
          <Button
            v-if="canManage"
            size="sm"
            :disabled="submitting || needsConfiguration"
            @click="emit('deploy', service.id)"
          >
            <Rocket data-icon="inline-start" :stroke-width="1.5" />
            {{ latestDeployment ? "Deploy again" : deployLabel }}
          </Button>
        </div>
      </div>

      <Alert v-if="latestDeployment?.failure_reason" variant="destructive">
        <CircleAlert :stroke-width="1.5" />
        <AlertTitle>Latest deployment failed</AlertTitle>
        <AlertDescription>{{ latestDeployment.failure_reason }}</AlertDescription>
      </Alert>

      <div v-if="serviceDeployments.length" class="divide-y divide-border border-y border-border">
        <div
          v-for="deployment in serviceDeployments"
          :key="deployment.id"
          class="flex items-center gap-3 py-3 max-[560px]:items-start max-[560px]:flex-col"
        >
          <button
            class="grid min-w-0 flex-1 grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 text-left"
            type="button"
            @click="selectDeployment(deployment)"
          >
            <span class="size-2 rounded-full" :class="statusDotClass(deployment.status)" />
            <span class="grid min-w-0 gap-1">
              <span class="text-sm font-medium">Generation {{ deployment.generation }}</span>
              <span class="truncate font-mono text-[11px] text-muted-foreground">
                {{ formatTimestamp(deployment.created_at) }}
              </span>
            </span>
            <Badge
              :variant="statusVariant(deployment.status)"
              :class="statusClass(deployment.status)"
            >
              {{ deployment.status }}
            </Badge>
          </button>
          <Button
            v-if="
              canManage && deployment.status === 'healthy' && deployment.id !== latestDeployment?.id
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
    </div>

    <div v-else class="p-5">
      <DeploymentLogsPanel :connected="connected" :logs="logs" :stream-error="streamError" />
    </div>
  </section>
</template>
