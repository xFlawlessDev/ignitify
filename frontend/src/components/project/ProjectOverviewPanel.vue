<script setup lang="ts">
import { Activity, Boxes, CircleAlert, RefreshCw, Rocket } from "@lucide/vue";
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { ActivitySummary, DeploymentSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  activity: ActivitySummary[];
  activityError: string | null;
  activityLoading: boolean;
  deploymentError: string | null;
  deployments: DeploymentSummary[];
  deploymentsLoading: boolean;
  serviceError: string | null;
  services: ServiceSummary[];
  servicesLoading: boolean;
}>();

const emit = defineEmits<{
  retryActivity: [];
  retryDeployments: [];
  retryServices: [];
}>();

const orderedDeployments = computed(() =>
  [...props.deployments].sort(
    (left, right) => new Date(right.created_at).getTime() - new Date(left.created_at).getTime(),
  ),
);
const latestDeploymentByService = computed(() => {
  const latest = new Map<string, DeploymentSummary>();
  for (const deployment of orderedDeployments.value) {
    if (!latest.has(deployment.service_id)) latest.set(deployment.service_id, deployment);
  }
  return latest;
});
const serviceNames = computed(
  () => new Map(props.services.map((service) => [service.id, service.name])),
);
const runningServiceCount = computed(
  () => props.services.filter((service) => service.desired_state === "running").length,
);
const activeDeploymentCount = computed(
  () =>
    props.deployments.filter((deployment) =>
      ["queued", "preparing", "running", "healthy", "stopping"].includes(deployment.status),
    ).length,
);
const failedLatestDeploymentCount = computed(
  () =>
    [...latestDeploymentByService.value.values()].filter(
      (deployment) => deployment.status === "failed",
    ).length,
);
const serviceRows = computed(() =>
  [...props.services]
    .sort((left, right) => left.name.localeCompare(right.name))
    .slice(0, 6)
    .map((service) => {
      const deployment = latestDeploymentByService.value.get(service.id);
      return {
        id: service.id,
        name: service.name,
        desiredState: service.desired_state,
        status: service.source_config?.setup_required
          ? "setup required"
          : (deployment?.status ?? "not deployed"),
      };
    }),
);
const recentDeployments = computed(() => orderedDeployments.value.slice(0, 5));
const recentActivity = computed(() => props.activity.slice(0, 4));

function statusClass(status: string) {
  if (["healthy", "running"].includes(status)) return "text-[var(--status-healthy)]";
  if (status === "failed") return "text-destructive";
  if (["queued", "preparing", "stopping"].includes(status)) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
</script>

<template>
  <div class="grid gap-6" aria-labelledby="project-overview-title">
    <header class="flex items-start justify-between gap-4 max-[560px]:flex-col">
      <div>
        <p class="ui-label">Control plane</p>
        <h2 id="project-overview-title" class="mt-2 text-2xl leading-none font-normal">
          Project overview
        </h2>
        <p class="mt-2 text-xs leading-5 text-muted-foreground">
          A quick read of this project's services, deployments, and recent changes.
        </p>
      </div>
      <Boxes class="size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
    </header>

    <section class="border-y border-border" aria-labelledby="overview-metrics-title">
      <h3 id="overview-metrics-title" class="sr-only">Project metrics</h3>
      <div class="grid grid-cols-2 divide-x divide-y divide-border md:grid-cols-4 md:divide-y-0">
        <div class="min-w-0 px-4 py-4">
          <p class="ui-label">Services</p>
          <Skeleton v-if="servicesLoading" class="mt-2 h-5 w-9" />
          <p v-else class="mt-2 font-mono text-xl">{{ services.length }}</p>
        </div>
        <div class="min-w-0 px-4 py-4">
          <p class="ui-label">Running target</p>
          <Skeleton v-if="servicesLoading" class="mt-2 h-5 w-9" />
          <p v-else class="mt-2 font-mono text-xl">{{ runningServiceCount }}</p>
        </div>
        <div class="min-w-0 px-4 py-4">
          <p class="ui-label">Active deployments</p>
          <Skeleton v-if="deploymentsLoading" class="mt-2 h-5 w-9" />
          <p v-else class="mt-2 font-mono text-xl">{{ activeDeploymentCount }}</p>
        </div>
        <div class="min-w-0 px-4 py-4">
          <p class="ui-label">Latest failures</p>
          <Skeleton v-if="deploymentsLoading" class="mt-2 h-5 w-9" />
          <p v-else class="mt-2 font-mono text-xl">{{ failedLatestDeploymentCount }}</p>
        </div>
      </div>
    </section>

    <section class="border border-border bg-card" aria-labelledby="service-status-title">
      <header
        class="flex items-end justify-between gap-4 border-b border-border px-5 py-4 max-[560px]:items-start max-[560px]:flex-col"
      >
        <div>
          <p class="ui-label">Deployment services</p>
          <h3 id="service-status-title" class="mt-2 text-lg font-normal">Service status</h3>
          <p class="mt-1 text-xs text-muted-foreground">
            Desired target and latest deployment result for each service.
          </p>
        </div>
        <span v-if="!servicesLoading" class="font-mono text-xs text-muted-foreground">
          {{ services.length }} total
        </span>
      </header>
      <div
        v-if="servicesLoading"
        class="divide-y divide-border"
        role="status"
        aria-label="Loading services"
      >
        <div v-for="index in 4" :key="index" class="grid gap-2 px-5 py-4">
          <Skeleton class="h-3 w-36 max-w-full" />
          <Skeleton class="h-2.5 w-24" />
        </div>
      </div>
      <div v-else-if="serviceError" class="px-5 py-5" role="alert">
        <p class="flex items-center gap-2 text-xs text-destructive">
          <CircleAlert class="size-3.5" :stroke-width="1.5" />
          {{ serviceError }}
        </p>
        <Button class="mt-3" size="sm" variant="outline" @click="emit('retryServices')">
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Retry
        </Button>
      </div>
      <div v-else-if="serviceRows.length">
        <div
          class="hidden grid-cols-[minmax(0,1fr)_10rem_10rem] gap-4 border-b border-border px-5 py-2.5 text-[10px] uppercase text-muted-foreground sm:grid"
        >
          <span>Service</span>
          <span>Target</span>
          <span>Latest result</span>
        </div>
        <div
          v-for="service in serviceRows"
          :key="service.id"
          class="grid gap-2 border-b border-border px-5 py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_10rem_10rem] sm:items-center sm:gap-4"
        >
          <p class="truncate text-sm font-medium">{{ service.name }}</p>
          <p class="text-xs text-muted-foreground">
            <span class="sm:hidden">Target: </span>
            <span class="capitalize">{{ service.desiredState }}</span>
          </p>
          <p class="text-xs capitalize" :class="statusClass(service.status)">
            <span class="sm:hidden">Latest: </span>{{ service.status }}
          </p>
        </div>
      </div>
      <div v-else class="px-5 py-8">
        <p class="text-sm font-medium">No services configured</p>
        <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
          Add a service to begin configuring a deployment source.
        </p>
      </div>
    </section>

    <div class="grid gap-6 xl:grid-cols-2">
      <section class="border border-border bg-card" aria-labelledby="latest-deployments-title">
        <header class="flex items-center justify-between gap-3 border-b border-border px-5 py-4">
          <div>
            <p class="ui-label">Runtime</p>
            <h3 id="latest-deployments-title" class="mt-2 text-lg font-normal">
              Latest deployments
            </h3>
          </div>
          <Rocket class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </header>
        <div
          v-if="deploymentsLoading"
          class="divide-y divide-border"
          role="status"
          aria-label="Loading deployments"
        >
          <div v-for="index in 4" :key="index" class="grid gap-2 px-5 py-4">
            <Skeleton class="h-3 w-36 max-w-full" />
            <Skeleton class="h-2.5 w-20" />
          </div>
        </div>
        <div v-else-if="deploymentError" class="px-5 py-5" role="alert">
          <p class="flex items-center gap-2 text-xs text-destructive">
            <CircleAlert class="size-3.5" :stroke-width="1.5" />
            {{ deploymentError }}
          </p>
          <Button class="mt-3" size="sm" variant="outline" @click="emit('retryDeployments')">
            <RefreshCw class="size-4" :stroke-width="1.5" />
            Retry
          </Button>
        </div>
        <div v-else-if="recentDeployments.length" class="divide-y divide-border">
          <div
            v-for="deployment in recentDeployments"
            :key="deployment.id"
            class="flex items-center justify-between gap-4 px-5 py-3.5"
          >
            <div class="min-w-0">
              <p class="truncate text-sm font-medium">
                {{ serviceNames.get(deployment.service_id) ?? "Unknown service" }}
              </p>
              <p class="mt-1 font-mono text-[11px] text-muted-foreground">
                g{{ deployment.generation }} · {{ formatTime(deployment.created_at) }}
              </p>
            </div>
            <span class="shrink-0 text-xs capitalize" :class="statusClass(deployment.status)">
              {{ deployment.status }}
            </span>
          </div>
        </div>
        <div v-else class="px-5 py-8">
          <p class="text-sm font-medium">No deployments yet</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            Deploy a configured service to create the first generation.
          </p>
        </div>
      </section>

      <section class="border border-border bg-card" aria-labelledby="recent-activity-title">
        <header class="flex items-center justify-between gap-3 border-b border-border px-5 py-4">
          <div>
            <p class="ui-label">Audit trail</p>
            <h3 id="recent-activity-title" class="mt-2 text-lg font-normal">Recent activity</h3>
          </div>
          <Activity class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </header>
        <div
          v-if="activityLoading"
          class="divide-y divide-border"
          role="status"
          aria-label="Loading activity"
        >
          <div v-for="index in 3" :key="index" class="grid gap-2 px-5 py-4">
            <Skeleton class="h-3 w-40 max-w-full" />
            <Skeleton class="h-2.5 w-24" />
          </div>
        </div>
        <div v-else-if="activityError" class="px-5 py-5" role="alert">
          <p class="flex items-center gap-2 text-xs text-destructive">
            <CircleAlert class="size-3.5" :stroke-width="1.5" />
            {{ activityError }}
          </p>
          <Button class="mt-3" size="sm" variant="outline" @click="emit('retryActivity')">
            <RefreshCw class="size-4" :stroke-width="1.5" />
            Retry
          </Button>
        </div>
        <div v-else-if="recentActivity.length" class="divide-y divide-border">
          <div v-for="item in recentActivity" :key="item.id" class="grid gap-1 px-5 py-3.5">
            <p class="truncate text-sm font-medium">{{ item.action }}</p>
            <p class="truncate font-mono text-[11px] text-muted-foreground">
              {{ item.resource_type ?? "workspace"
              }}<span v-if="item.resource_id"> · {{ item.resource_id }}</span>
            </p>
            <time class="font-mono text-[10px] text-muted-foreground" :datetime="item.created_at">
              {{ formatTime(item.created_at) }}
            </time>
          </div>
        </div>
        <div v-else class="px-5 py-8 text-sm text-muted-foreground">No project activity yet.</div>
      </section>
    </div>
  </div>
</template>
