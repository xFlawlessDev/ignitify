<script setup lang="ts">
import { ArrowUpRight, Box, CircleAlert, Layers3, RefreshCw, Rocket } from "@lucide/vue";
import { onMounted } from "vue";
import { RouterLink } from "vue-router";
import DeploymentList from "@/components/dashboard/DeploymentList.vue";
import MetricTile from "@/components/dashboard/MetricTile.vue";
import RuntimeStatusPanel from "@/components/runtime/RuntimeStatusPanel.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useOperationsDashboard } from "@/composables/useOperationsDashboard";

const { data, error, load, loading, metrics, recentDeployments, runtime } =
  useOperationsDashboard();

onMounted(load);
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
    >
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Operations</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Workspace rollout state. Deployment status from persisted control-plane records.
        </p>
      </div>
      <Button as-child class="w-full sm:w-auto" size="sm">
        <RouterLink to="/projects">
          <Box class="size-4" :stroke-width="1.5" />
          Open projects
        </RouterLink>
      </Button>
    </header>

    <section
      class="mt-[22px] grid overflow-hidden divide-y divide-border border border-border bg-card sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
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

    <section
      v-if="error"
      class="mt-4 flex items-start justify-between gap-4 border border-destructive/40 bg-card px-5 py-4 max-[640px]:flex-col"
      role="alert"
    >
      <div class="flex items-start gap-2 text-sm text-destructive">
        <CircleAlert class="mt-0.5 size-4 shrink-0" :stroke-width="1.5" />
        <p>{{ error }}</p>
      </div>
      <Button
        class="shrink-0 max-[640px]:w-full"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="load"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Retry
      </Button>
    </section>

    <section class="mt-[22px] grid min-w-0 gap-4 lg:grid-cols-[minmax(0,1fr)_17rem]">
      <DeploymentList :deployments="recentDeployments" :loading="loading" />
      <RuntimeStatusPanel :runtime="runtime" :loading="loading && !runtime" />
    </section>

    <section
      v-if="!loading && !data.projects.length"
      class="mt-4 border border-border bg-card px-5 py-5"
    >
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
