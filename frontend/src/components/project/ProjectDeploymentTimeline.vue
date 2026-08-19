<script setup lang="ts">
import {
  ChevronDown,
  CircleAlert,
  LoaderCircle,
  Rocket,
  RotateCcw,
  ShieldCheck,
  Square,
} from "@lucide/vue";
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { DeploymentState, DeploymentSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  deployments: DeploymentSummary[];
  error: string | null;
  loading: boolean;
  services: ServiceSummary[];
  submitting: boolean;
  canApprove?: boolean;
  selectedDeploymentId?: string | null;
}>();

const emit = defineEmits<{
  deploy: [serviceId: string];
  stop: [serviceId: string];
  rollback: [deploymentId: string];
  approve: [deploymentId: string];
  select: [deploymentId: string];
  retry: [];
}>();

function serviceName(serviceId: string) {
  return props.services.find((service) => service.id === serviceId)?.name ?? "Unknown service";
}

function isActive(status: DeploymentSummary["status"]) {
  return ["queued", "preparing", "running", "stopping"].includes(status);
}

const statusLabels: Record<DeploymentState, string> = {
  failed: "Failed",
  healthy: "Healthy",
  preparing: "Preparing",
  queued: "Queued",
  running: "Running",
  stopped: "Stopped",
  stopping: "Stopping",
  superseded: "Superseded",
};

const latestDeploymentIds = computed(() => {
  const ids = new Set<string>();
  const services = new Set<string>();
  for (const deployment of props.deployments) {
    if (!services.has(deployment.service_id)) {
      services.add(deployment.service_id);
      ids.add(deployment.id);
    }
  }
  return ids;
});

function statusClass(status: DeploymentState) {
  if (status === "failed") return "text-destructive";
  if (status === "healthy") return "text-[var(--status-healthy)]";
  if (isActive(status)) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function statusDotState(status: DeploymentState) {
  if (status === "healthy") return "healthy";
  if (status === "failed") return "failed";
  if (isActive(status)) return "live";
  return "inactive";
}

function statusLabel(deployment: DeploymentSummary) {
  return deployment.approval.status === "pending"
    ? "Awaiting approval"
    : statusLabels[deployment.status];
}

function formatTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  return new Intl.DateTimeFormat(undefined, {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    timeZone: "UTC",
  }).format(date);
}
</script>

<template>
  <section class="app-surface">
    <header
      class="app-panel-header flex items-end justify-between gap-4 px-5 py-4 max-[560px]:items-start max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Runtime history</p>
        <div class="mt-2 flex items-baseline gap-3">
          <h2 class="text-lg font-normal">Deployments</h2>
          <span
            v-if="!loading && deployments.length"
            class="font-mono text-[11px] text-muted-foreground"
          >
            {{ deployments.length }} recorded
          </span>
        </div>
      </div>
      <DropdownMenu>
        <DropdownMenuTrigger as-child>
          <Button
            class="max-[560px]:w-full"
            size="sm"
            type="button"
            :disabled="submitting || !services.length"
          >
            <Rocket class="size-4" :stroke-width="1.5" />
            Deploy service
            <ChevronDown class="size-3.5" :stroke-width="1.5" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" class="w-60">
          <DropdownMenuLabel>Select a service</DropdownMenuLabel>
          <DropdownMenuItem
            v-for="service in services"
            :key="service.id"
            :disabled="submitting"
            @select="emit('deploy', service.id)"
          >
            <Rocket class="size-4" :stroke-width="1.5" />
            <span class="truncate">{{ service.name }}</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </header>

    <div
      v-if="loading"
      class="divide-y divide-border"
      role="status"
      aria-label="Loading deployments"
    >
      <div
        v-for="index in 4"
        :key="index"
        class="grid min-w-0 gap-3 px-5 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center"
      >
        <div class="flex min-w-0 items-start gap-3">
          <Skeleton class="mt-1 size-3 shrink-0 rounded-full" />
          <div class="grid min-w-0 flex-1 gap-2">
            <Skeleton class="h-3 w-40 max-w-full" />
            <Skeleton class="h-2.5 w-24 max-w-full" />
          </div>
        </div>
        <div class="flex items-center gap-4 pl-6 md:pl-0">
          <Skeleton class="h-2.5 w-16" />
          <Skeleton class="h-2.5 w-28" />
          <Skeleton class="size-8 rounded-sm" />
        </div>
      </div>
    </div>
    <section v-else-if="error && !deployments.length" class="px-5 py-5" role="alert">
      <p class="flex items-center gap-2 text-sm text-destructive">
        <CircleAlert class="size-4" :stroke-width="1.5" />
        {{ error }}
      </p>
      <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
    </section>
    <div v-else-if="deployments.length" class="divide-y divide-border">
      <section v-if="error" class="border-b border-border px-5 py-3" role="alert">
        <p class="flex items-center gap-2 text-sm text-destructive">
          <CircleAlert class="size-4" :stroke-width="1.5" />
          {{ error }}
        </p>
        <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
      </section>
      <article
        v-for="(deployment, index) in deployments"
        :key="deployment.id"
        class="relative grid min-w-0 cursor-pointer gap-3 px-5 py-4 transition-colors hover:bg-muted/35 focus-visible:bg-muted/50 focus-visible:outline-none md:grid-cols-[minmax(0,1fr)_auto] md:items-center"
        :class="props.selectedDeploymentId === deployment.id ? 'bg-muted/55' : ''"
        :aria-current="props.selectedDeploymentId === deployment.id ? 'true' : undefined"
        tabindex="0"
        @click="emit('select', deployment.id)"
        @keydown.enter.prevent="emit('select', deployment.id)"
        @keydown.space.prevent="emit('select', deployment.id)"
      >
        <span
          v-if="index < deployments.length - 1"
          class="absolute top-8 bottom-0 left-[26px] w-px bg-border"
          aria-hidden="true"
        />
        <div class="flex min-w-0 items-start gap-3">
          <span class="mt-1.5 size-3 shrink-0 rounded-full border-[3px] border-card bg-background">
            <span
              class="status-dot block size-1.5"
              :data-status="statusDotState(deployment.status)"
              aria-hidden="true"
            />
          </span>
          <div class="min-w-0">
            <p class="truncate text-sm font-medium">{{ serviceName(deployment.service_id) }}</p>
            <div
              class="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground"
            >
              <span class="font-mono text-[11px]">g{{ deployment.generation }}</span>
              <span aria-hidden="true">&#183;</span>
              <span
                >{{ deployment.attempt_count }} attempt{{
                  deployment.attempt_count === 1 ? "" : "s"
                }}</span
              >
            </div>
            <p
              v-if="deployment.failure_reason"
              class="mt-2 line-clamp-2 break-words text-xs text-destructive"
              :title="deployment.failure_reason"
            >
              {{ deployment.failure_reason }}
            </p>
          </div>
        </div>
        <div class="flex flex-wrap items-center gap-x-4 gap-y-2 pl-6 md:justify-end md:pl-0">
          <span class="flex items-center gap-2 text-xs" :class="statusClass(deployment.status)">
            <CircleAlert
              v-if="deployment.status === 'failed'"
              class="size-3.5"
              :stroke-width="1.5"
            />
            <LoaderCircle
              v-else-if="isActive(deployment.status)"
              class="size-3.5 animate-spin"
              :stroke-width="1.5"
            />
            <span
              v-else
              class="status-dot"
              :data-status="statusDotState(deployment.status)"
              aria-hidden="true"
            />
            {{ statusLabel(deployment) }}
          </span>
          <time
            class="shrink-0 font-mono text-[10px] uppercase text-muted-foreground/80"
            :datetime="deployment.created_at"
            :title="deployment.created_at"
          >
            {{ formatTime(deployment.created_at) }} UTC
          </time>
          <div
            v-if="
              deployment.status === 'healthy' ||
              deployment.status === 'running' ||
              deployment.status === 'stopped' ||
              deployment.status === 'superseded'
            "
            class="flex items-center gap-1"
          >
            <Tooltip v-if="deployment.approval.status === 'pending' && canApprove">
              <TooltipTrigger as-child>
                <Button
                  class="app-action-icon"
                  size="icon-sm"
                  variant="ghost"
                  type="button"
                  :aria-label="`Approve deployment ${deployment.generation}`"
                  :disabled="submitting"
                  @click.stop="emit('approve', deployment.id)"
                >
                  <ShieldCheck class="size-4" :stroke-width="1.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Approve production deployment</TooltipContent>
            </Tooltip>
            <Tooltip v-if="deployment.status === 'healthy' || deployment.status === 'running'">
              <TooltipTrigger as-child>
                <Button
                  class="app-action-icon"
                  size="icon-sm"
                  variant="ghost"
                  type="button"
                  :aria-label="`Stop ${serviceName(deployment.service_id)}`"
                  :disabled="submitting"
                  @click="emit('stop', deployment.service_id)"
                >
                  <Square class="size-4" :stroke-width="1.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Stop service</TooltipContent>
            </Tooltip>
            <Tooltip
              v-if="
                !latestDeploymentIds.has(deployment.id) &&
                (deployment.status === 'healthy' ||
                  deployment.status === 'stopped' ||
                  deployment.status === 'superseded')
              "
            >
              <TooltipTrigger as-child>
                <Button
                  class="app-action-icon"
                  size="icon-sm"
                  variant="ghost"
                  type="button"
                  :aria-label="`Rollback deployment ${deployment.generation}`"
                  :disabled="submitting"
                  @click="emit('rollback', deployment.id)"
                >
                  <RotateCcw class="size-4" :stroke-width="1.5" />
                </Button>
              </TooltipTrigger>
              <TooltipContent>Rollback deployment</TooltipContent>
            </Tooltip>
          </div>
        </div>
      </article>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No deployments yet</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Deploy a configured service when it is ready to run.
      </p>
    </div>
  </section>
</template>
