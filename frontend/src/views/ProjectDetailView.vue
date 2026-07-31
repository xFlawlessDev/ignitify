<script setup lang="ts">
import {
  ArrowLeft,
  Check,
  ExternalLink,
  GitBranch,
  Globe,
  MoreHorizontal,
  Rocket,
  Settings2,
} from "@lucide/vue";
import { shallowRef } from "vue";
import { RouterLink } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import ProjectDeploymentTimeline from "@/components/project/ProjectDeploymentTimeline.vue";
import ProjectEnvironmentPanel from "@/components/project/ProjectEnvironmentPanel.vue";
import ProjectServiceList, {
  type ProjectService,
} from "@/components/project/ProjectServiceList.vue";

const activeTab = shallowRef("overview");
const actionMessage = shallowRef("");

const services: ProjectService[] = [
  {
    id: "nova-api",
    name: "nova-api",
    type: "Web service",
    branch: "main",
    status: "healthy",
    commit: "a91c7f2",
    deployedAt: "12 min ago",
  },
  {
    id: "nova-worker",
    name: "nova-worker",
    type: "Worker",
    branch: "main",
    status: "healthy",
    commit: "a91c7f2",
    deployedAt: "12 min ago",
  },
  {
    id: "nova-web",
    name: "nova-web",
    type: "Static site",
    branch: "main",
    status: "building",
    commit: "8e3bd11",
    deployedAt: "Building now",
  },
];

const deployments = [
  {
    id: "deploy-1",
    service: "nova-api",
    commit: "a91c7f2",
    actor: "Arif",
    time: "12 min ago",
    status: "success" as const,
  },
  {
    id: "deploy-2",
    service: "nova-worker",
    commit: "a91c7f2",
    actor: "Arif",
    time: "14 min ago",
    status: "success" as const,
  },
  {
    id: "deploy-3",
    service: "nova-web",
    commit: "8e3bd11",
    actor: "CI pipeline",
    time: "Now",
    status: "building" as const,
  },
  {
    id: "deploy-4",
    service: "nova-api",
    commit: "2ad81fc",
    actor: "Arif",
    time: "Yesterday",
    status: "failed" as const,
  },
];

const variables = [
  { key: "NODE_ENV", value: "production", secret: false },
  { key: "DATABASE_URL", value: "postgres://nova:••••@db.internal", secret: true },
  { key: "REDIS_URL", value: "redis://cache.internal:6379", secret: true },
  { key: "LOG_LEVEL", value: "info", secret: false },
];

function queueAction(action: string) {
  actionMessage.value = `${action} queued for Nova API`;
  window.setTimeout(() => {
    actionMessage.value = "";
  }, 2400);
}
</script>

<template>
  <div class="max-w-[1160px]">
    <RouterLink
      class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
      to="/projects"
    >
      <ArrowLeft :size="15" :stroke-width="1.5" />
      Projects
    </RouterLink>

    <header
      class="mt-[22px] flex items-center justify-between gap-6 border-b border-border pb-[25px] max-[620px]:items-start max-[620px]:flex-col"
    >
      <div class="flex min-w-0 items-center gap-[13px]">
        <div
          class="grid size-11 shrink-0 place-items-center rounded-[5px] bg-[#d7a158] font-mono text-xs font-semibold text-[#30251c]"
        >
          NF
        </div>
        <div>
          <div class="flex flex-wrap items-center gap-2.5">
            <h1 class="m-0 text-[29px] leading-none font-normal">Nova API</h1>
            <span
              class="inline-flex items-center gap-1.5 font-mono text-[10px] text-[var(--status-healthy)] uppercase"
              ><span class="status-dot" data-status="healthy" /> Live</span
            >
          </div>
          <p class="mt-2 text-xs text-muted-foreground">
            Production project · updated 12 minutes ago
          </p>
        </div>
      </div>
      <div class="flex shrink-0 gap-2 max-[620px]:w-full">
        <button
          class="flex min-h-8 items-center gap-2 rounded-[3px] border border-border bg-card px-3 text-xs text-foreground hover:border-input hover:bg-muted max-[620px]:flex-1 max-[620px]:justify-center"
          type="button"
          @click="queueAction('Deploy')"
        >
          <Rocket :size="15" :stroke-width="1.5" />
          Deploy
        </button>
        <button
          class="grid size-8 place-items-center rounded-[3px] border border-border bg-card text-muted-foreground hover:border-input hover:bg-muted"
          type="button"
          aria-label="Open project actions"
          @click="queueAction('More actions')"
        >
          <MoreHorizontal :size="17" :stroke-width="1.5" />
        </button>
      </div>
    </header>

    <div
      v-if="actionMessage"
      class="mt-4 flex items-center gap-2 border border-[color-mix(in_srgb,var(--status-live)_22%,var(--border))] bg-[color-mix(in_srgb,var(--status-live)_8%,transparent)] px-3 py-2.5 font-mono text-[11px] text-[var(--status-live)]"
      role="status"
    >
      <Check :size="15" :stroke-width="1.8" />
      {{ actionMessage }}
    </div>

    <Tabs v-model="activeTab" class="mt-[25px]">
      <TabsList
        class="flex h-[39px] w-full justify-start gap-1 overflow-x-auto rounded-none border-b border-border bg-transparent p-0"
      >
        <TabsTrigger
          value="overview"
          class="h-[39px] flex-none rounded-none border-0 border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground shadow-none data-[state=active]:border-b-[var(--status-live)] data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none"
          >Overview</TabsTrigger
        >
        <TabsTrigger
          value="deployments"
          class="h-[39px] flex-none rounded-none border-0 border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground shadow-none data-[state=active]:border-b-[var(--status-live)] data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none"
          >Deployments
          <span class="font-mono text-[10px] text-muted-foreground">4</span></TabsTrigger
        >
        <TabsTrigger
          value="environment"
          class="h-[39px] flex-none rounded-none border-0 border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground shadow-none data-[state=active]:border-b-[var(--status-live)] data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none"
          >Environment</TabsTrigger
        >
        <TabsTrigger
          value="settings"
          class="h-[39px] flex-none rounded-none border-0 border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground shadow-none data-[state=active]:border-b-[var(--status-live)] data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none"
          ><Settings2 :size="14" :stroke-width="1.5" /> Settings</TabsTrigger
        >
      </TabsList>

      <TabsContent value="overview" class="pt-[22px]">
        <section
          class="grid grid-cols-4 border border-border bg-card max-[900px]:grid-cols-2 max-[620px]:grid-cols-1"
        >
          <div
            v-for="(item, index) in [
              {
                label: 'Status',
                value: 'Operational',
                detail: '99.98% uptime this month',
                icon: null,
                healthy: true,
              },
              {
                label: 'Source',
                value: 'main',
                detail: 'github.com/novaflow/api',
                icon: GitBranch,
              },
              { label: 'Region', value: 'Singapore', detail: 'fra1 · shared cluster', icon: Globe },
              {
                label: 'Primary URL',
                value: 'api.novaflow.dev',
                detail: 'HTTPS · managed certificate',
                icon: ExternalLink,
              },
            ]"
            :key="item.label"
            class="grid min-h-[104px] gap-2.5 border-r border-border px-[18px] py-4 last:border-r-0 max-[900px]:nth-[2]:border-r-0 max-[900px]:nth-[-n+2]:border-b max-[620px]:border-r-0 max-[620px]:border-b max-[620px]:last:border-b-0"
          >
            <span class="ui-label">{{ item.label }}</span>
            <strong
              class="flex min-w-0 items-center gap-1.5 truncate text-[13px] font-medium"
              :class="item.healthy ? 'text-[var(--status-healthy)]' : ''"
            >
              <span v-if="item.healthy" class="status-dot" data-status="healthy" />
              <component
                :is="item.icon"
                v-else
                :size="14"
                :stroke-width="1.5"
                class="shrink-0 text-muted-foreground"
              />
              {{ item.value }}
            </strong>
            <small class="truncate text-[11px] text-muted-foreground">{{ item.detail }}</small>
          </div>
        </section>

        <ProjectServiceList class="mt-[22px]" :services="services" />
        <div
          class="mt-[22px] grid grid-cols-[minmax(0,1.15fr)_minmax(320px,0.85fr)] gap-[22px] max-[900px]:grid-cols-1"
        >
          <ProjectDeploymentTimeline :deployments="deployments.slice(0, 3)" />
          <ProjectEnvironmentPanel :variables="variables.slice(0, 3)" />
        </div>
      </TabsContent>

      <TabsContent value="deployments" class="pt-[22px]"
        ><ProjectDeploymentTimeline :deployments="deployments"
      /></TabsContent>
      <TabsContent value="environment" class="pt-[22px]">
        <div
          class="grid grid-cols-[minmax(0,1.3fr)_minmax(280px,0.7fr)] gap-[22px] max-[900px]:grid-cols-1"
        >
          <ProjectEnvironmentPanel :variables="variables" />
          <section class="flex items-start gap-3.5 border border-border bg-card p-[22px]">
            <div>
              <p class="ui-label">Access policy</p>
              <h2 class="mt-2.5 mb-[7px] text-lg font-normal">Runtime values stay private</h2>
              <p class="m-0 max-w-[360px] text-xs leading-[1.6] text-muted-foreground">
                Secrets are injected during deploy and never exposed in build logs.
              </p>
            </div>
          </section>
        </div>
      </TabsContent>
      <TabsContent value="settings" class="pt-[22px]">
        <section class="flex items-start gap-3.5 border border-border bg-card p-[22px]">
          <Settings2 class="shrink-0 text-muted-foreground" :size="20" :stroke-width="1.4" />
          <div>
            <p class="ui-label">Project settings</p>
            <h2 class="mt-2.5 mb-[7px] text-lg font-normal">Configuration controls arrive next</h2>
            <p class="m-0 max-w-[360px] text-xs leading-[1.6] text-muted-foreground">
              Repository, build command, domains, and deletion guard will live here.
            </p>
          </div>
        </section>
      </TabsContent>
    </Tabs>
  </div>
</template>
