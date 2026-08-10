<script setup lang="ts">
import { ArrowUpRight, Box, CircleAlert, Layers3, RefreshCw, Rocket } from "@lucide/vue";
import { onMounted } from "vue";
import { toast } from "vue-sonner";
import { RouterLink } from "vue-router";
import DeploymentList from "@/components/dashboard/DeploymentList.vue";
import MetricTile from "@/components/dashboard/MetricTile.vue";
import RuntimeStatusPanel from "@/components/runtime/RuntimeStatusPanel.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useOperationsDashboard } from "@/composables/useOperationsDashboard";

const { data, error, load, loading, metrics, recentDeployments, runtime } =
  useOperationsDashboard();

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
        <p class="mt-2 text-sm text-muted-foreground">
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
            Open projects
          </RouterLink>
        </Button>
      </div>
    </header>

    <section
      class="mt-6 app-surface grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
    >
      <template v-if="loading && !data.projects.length">
        <div v-for="index in 4" :key="index" class="min-h-36 bg-background p-5">
          <Skeleton class="h-3 w-20" />
          <Skeleton class="mt-12 h-9 w-12" />
        </div>
      </template>
      <template v-else>
        <MetricTile label="Projects" :value="String(metrics.projects)" :icon="Box" />
        <MetricTile label="Services" :value="String(metrics.services)" :icon="Layers3" />
        <MetricTile label="In progress" :value="String(metrics.active)" :icon="Rocket" />
        <MetricTile label="Needs attention" :value="String(metrics.failed)" :icon="CircleAlert" />
      </template>
    </section>

    <section class="mt-6 grid min-w-0 gap-6 lg:grid-cols-[minmax(0,1fr)_17rem]">
      <DeploymentList :deployments="recentDeployments" :loading="loading" />
      <RuntimeStatusPanel :runtime="runtime" :loading="loading && !runtime" />
    </section>

    <section v-if="!loading && !data.projects.length" class="mt-4 app-surface px-5 py-5">
      <p class="text-sm font-medium">Start with a project</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Projects contain environments, services, domains, and deployment history.
      </p>
      <RouterLink
        class="mt-4 inline-flex items-center gap-1 text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
        to="/projects"
      >
        Create project
        <ArrowUpRight class="size-3.5" :stroke-width="1.5" />
      </RouterLink>
    </section>
  </div>
</template>
