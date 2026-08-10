<script setup lang="ts">
import {
  Box,
  CircleAlert,
  CircleCheckBig,
  Layers3,
  Plus,
  RefreshCw,
  Rocket,
  Server,
} from "@lucide/vue";
import { computed, onMounted } from "vue";
import { toast } from "vue-sonner";
import { RouterLink } from "vue-router";
import DeploymentList from "@/components/dashboard/DeploymentList.vue";
import MetricTile from "@/components/dashboard/MetricTile.vue";
import OperationsTopology from "@/components/dashboard/OperationsTopology.vue";
import RuntimeStatusPanel from "@/components/runtime/RuntimeStatusPanel.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useOperationsDashboard } from "@/composables/useOperationsDashboard";

const { data, error, load, loading, metrics, recentDeployments, runtime } =
  useOperationsDashboard();

const workspaceStatus = computed(() => {
  if (error.value) {
    return {
      detail: "Some workspace signals could not be loaded.",
      label: "Partial visibility",
      tone: "warning",
    };
  }
  if (metrics.value.failed > 0) {
    return {
      detail: `${metrics.value.failed} latest deployment${metrics.value.failed === 1 ? "" : "s"} failed.`,
      label: "Action required",
      tone: "failed",
    };
  }
  if (metrics.value.active > 0) {
    return {
      detail: `${metrics.value.active} deployment${metrics.value.active === 1 ? "" : "s"} in progress.`,
      label: "Deployment activity",
      tone: "live",
    };
  }
  if (metrics.value.healthy > 0) {
    return {
      detail: `${metrics.value.healthy} service${metrics.value.healthy === 1 ? "" : "s"} reporting healthy.`,
      label: "Workspace stable",
      tone: "healthy",
    };
  }
  if (metrics.value.services > 0) {
    return {
      detail: "Services are configured and awaiting their next deployment.",
      label: "Ready for a release",
      tone: "neutral",
    };
  }
  return {
    detail: "Create a project to begin configuring services.",
    label: "Workspace ready",
    tone: "neutral",
  };
});

const runtimeStatus = computed(() => {
  if (!runtime.value) {
    return {
      detail: "Runtime readiness is unavailable.",
      label: "Runtime unavailable",
      tone: "warning",
    };
  }
  const ready = [runtime.value.database, runtime.value.runtime, runtime.value.worker].every(
    (status) => status === "ready",
  );
  return ready
    ? {
        detail: "Database, runtime, and worker are ready.",
        label: "Runtime ready",
        tone: "healthy",
      }
    : {
        detail: "One or more runtime components require attention.",
        label: "Runtime degraded",
        tone: "warning",
      };
});

const operationsLabel = computed(
  () =>
    `${data.value.deployments.length} recorded operation${data.value.deployments.length === 1 ? "" : "s"}`,
);

async function loadDashboard(showSuccess = false) {
  await load();
  if (error.value) {
    toast.error("Operations unavailable", { description: error.value });
    return;
  }
  if (showSuccess) toast.success("Operations refreshed");
}

onMounted(() => void loadDashboard());
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Operations</h1>
        <p class="mt-2 max-w-[58ch] text-sm leading-5 text-muted-foreground">
          Workspace rollout state. Deployment status from persisted control-plane records.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <Button
          class="shrink-0"
          size="icon-sm"
          variant="outline"
          :disabled="loading"
          aria-label="Refresh operations"
          title="Refresh operations"
          @click="loadDashboard(true)"
        >
          <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        </Button>
        <Button as-child class="min-w-0 flex-1 sm:w-auto" size="sm">
          <RouterLink to="/projects">
            <Box class="size-4" :stroke-width="1.5" />
            Manage projects
          </RouterLink>
        </Button>
      </div>
    </header>

    <section
      class="mt-4 grid divide-y divide-border border-y border-border sm:grid-cols-2 sm:divide-x sm:divide-y-0"
      aria-label="Workspace and runtime status"
      aria-live="polite"
    >
      <div class="flex min-w-0 items-start gap-3 px-0 py-3.5 sm:pr-6">
        <span class="status-dot mt-1.5" :data-status="workspaceStatus.tone" aria-hidden="true" />
        <div class="min-w-0">
          <p class="text-sm font-medium">{{ workspaceStatus.label }}</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">{{ workspaceStatus.detail }}</p>
        </div>
      </div>
      <div class="flex min-w-0 items-start gap-3 px-0 py-3.5 sm:px-6">
        <Server class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div class="min-w-0">
          <p class="text-sm font-medium">{{ runtimeStatus.label }}</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">{{ runtimeStatus.detail }}</p>
        </div>
      </div>
    </section>

    <section
      v-if="error"
      class="mt-4 flex items-start gap-3 border-y border-destructive/40 py-3.5 text-sm"
      role="alert"
    >
      <CircleAlert class="mt-0.5 size-4 shrink-0 text-destructive" :stroke-width="1.5" />
      <div>
        <p class="font-medium">Some operations data is unavailable</p>
        <p class="mt-1 text-xs leading-5 text-muted-foreground">{{ error }}</p>
      </div>
    </section>

    <section class="mt-6" aria-labelledby="workspace-overview-title">
      <div class="flex flex-wrap items-end justify-between gap-3">
        <div>
          <p class="ui-label">Workspace overview</p>
          <h2 id="workspace-overview-title" class="mt-2 text-base font-medium">
            Deployment health at a glance
          </h2>
        </div>
        <p class="font-mono text-[11px] text-muted-foreground">{{ operationsLabel }}</p>
      </div>
      <div
        class="mt-3 app-surface grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
      >
        <template v-if="loading && !data.projects.length">
          <div v-for="index in 4" :key="index" class="min-h-36 bg-background p-5">
            <Skeleton class="h-3 w-20" />
            <Skeleton class="mt-12 h-9 w-12" />
          </div>
        </template>
        <template v-else>
          <MetricTile
            label="Projects"
            :value="String(metrics.projects)"
            :detail="`${metrics.services} configured service${metrics.services === 1 ? '' : 's'}`"
            :icon="Box"
          />
          <MetricTile
            label="Services"
            :value="String(metrics.services)"
            :detail="`${metrics.projects} project${metrics.projects === 1 ? '' : 's'} in workspace`"
            :icon="Layers3"
          />
          <MetricTile
            label="In progress"
            :value="String(metrics.active)"
            :detail="metrics.active ? 'Queued, preparing, or running' : 'No active deployments'"
            :icon="Rocket"
            :tone="metrics.active ? 'live' : 'neutral'"
          />
          <MetricTile
            label="Needs attention"
            :value="String(metrics.failed)"
            :detail="metrics.failed ? 'Failed latest deployment' : 'No failed latest deployment'"
            :icon="metrics.failed ? CircleAlert : CircleCheckBig"
            :tone="metrics.failed ? 'destructive' : 'healthy'"
          />
        </template>
      </div>
    </section>

    <OperationsTopology
      v-if="loading || data.projects.length"
      :deployments="data.deployments"
      :loading="loading"
      :projects="data.projects"
      :runtime="runtime"
      :services="data.services"
    />

    <section
      v-if="!loading && !error && !data.projects.length"
      class="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]"
    >
      <article
        class="app-surface flex min-h-72 flex-col items-start justify-center px-5 py-8 sm:px-8"
      >
        <span class="grid size-9 place-items-center rounded-sm bg-muted text-muted-foreground">
          <Box class="size-4" :stroke-width="1.5" />
        </span>
        <p class="ui-label mt-6">Workspace setup</p>
        <h2 class="mt-2 text-lg font-medium">Create your first project</h2>
        <p class="mt-2 max-w-md text-sm leading-5 text-muted-foreground">
          Begin by adding the project that will contain your deployment services.
        </p>
        <Button as-child class="mt-6" size="sm">
          <RouterLink to="/projects">
            <Plus class="size-4" :stroke-width="1.5" />
            Create project
          </RouterLink>
        </Button>
      </article>
      <RuntimeStatusPanel :runtime="runtime" :loading="loading && !runtime" />
    </section>

    <section v-else class="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_19rem]">
      <DeploymentList :deployments="recentDeployments" :loading="loading" />
      <RuntimeStatusPanel :runtime="runtime" :loading="loading && !runtime" />
    </section>
  </div>
</template>
