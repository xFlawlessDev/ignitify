<script setup lang="ts">
import { ArrowUpRight, CircleAlert, GitBranch, LoaderCircle } from "@lucide/vue";
import { RouterLink } from "vue-router";
import type { DashboardDeployment } from "@/composables/useOperationsDashboard";
import type { DeploymentState } from "@/lib/types";

interface Props {
  deployments: DashboardDeployment[];
  loading: boolean;
}

const props = defineProps<Props>();

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

function isActive(status: DeploymentState) {
  return ["queued", "preparing", "running", "stopping"].includes(status);
}

function statusClass(status: DeploymentState) {
  if (status === "failed") return "text-destructive";
  if (status === "healthy") return "text-[var(--status-healthy)]";
  if (isActive(status)) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function formatTime(value: string) {
  return new Intl.DateTimeFormat("en-GB", {
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    month: "short",
    timeZone: "UTC",
  }).format(new Date(value));
}
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-end justify-between gap-4 border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Deployments</p>
        <h2 class="mt-2 text-base font-medium">Recent operations</h2>
      </div>
      <RouterLink
        class="inline-flex shrink-0 items-center gap-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
        to="/projects"
      >
        Projects
        <ArrowUpRight class="size-3.5" :stroke-width="1.5" />
      </RouterLink>
    </div>

    <p v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" role="status">
      Loading recent operations...
    </p>
    <div v-else-if="props.deployments.length" class="divide-y divide-border">
      <RouterLink
        v-for="item in props.deployments"
        :key="item.deployment.id"
        :to="`/projects/${item.project.id}`"
        class="grid gap-3 px-5 py-4 hover:bg-muted/60 sm:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_auto] sm:items-center"
      >
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-sm bg-muted text-muted-foreground"
          >
            <GitBranch class="size-4" :stroke-width="1.5" />
          </span>
          <div class="min-w-0">
            <p class="truncate text-sm font-medium">
              {{ item.service?.name ?? "Removed service" }}
            </p>
            <p class="mt-1 truncate text-xs text-muted-foreground">{{ item.project.name }}</p>
          </div>
        </div>
        <div class="min-w-0 text-xs text-muted-foreground">
          <p class="font-mono text-[11px]">g{{ item.deployment.generation }}</p>
          <p v-if="item.deployment.failure_reason" class="mt-1 truncate text-destructive">
            {{ item.deployment.failure_reason }}
          </p>
        </div>
        <div class="flex items-center justify-between gap-4 sm:justify-end">
          <span
            class="flex items-center gap-2 text-xs"
            :class="statusClass(item.deployment.status)"
          >
            <CircleAlert
              v-if="item.deployment.status === 'failed'"
              class="size-3.5"
              :stroke-width="1.5"
            />
            <LoaderCircle
              v-else-if="isActive(item.deployment.status)"
              class="size-3.5 animate-spin"
              :stroke-width="1.5"
            />
            <span
              v-else
              class="status-dot"
              :data-status="item.deployment.status === 'healthy' ? 'healthy' : undefined"
              aria-hidden="true"
            />
            {{ statusLabels[item.deployment.status] }}
          </span>
          <time
            class="shrink-0 font-mono text-[10px] uppercase text-muted-foreground/70"
            :datetime="item.deployment.created_at"
          >
            {{ formatTime(item.deployment.created_at) }} UTC
          </time>
        </div>
      </RouterLink>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No deployments recorded</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Configure a service from its project, then submit its first deployment.
      </p>
    </div>
  </section>
</template>
