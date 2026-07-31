<script setup lang="ts">
import {
  ArrowUpRight,
  Box,
  Check,
  CircleAlert,
  Clock3,
  Cpu,
  GitBranch,
  HardDrive,
  MoreHorizontal,
  Server,
} from "@lucide/vue";
import { computed, shallowRef } from "vue";
import { RouterLink } from "vue-router";
import { Button } from "@/components/ui/button";

type Health = "healthy" | "attention" | "inactive";
type DeploymentState = "Live" | "Building" | "Failed";

interface Project {
  id: string;
  name: string;
  source: string;
  service: string;
  status: DeploymentState;
  branch: string;
  updated: string;
}

const projects: Project[] = [
  {
    id: "api",
    name: "core-api",
    source: "github.com/novaflow/core-api",
    service: "API service",
    status: "Live",
    branch: "main",
    updated: "2 min ago",
  },
  {
    id: "web",
    name: "dashboard-web",
    source: "github.com/novaflow/dashboard",
    service: "Web service",
    status: "Building",
    branch: "main",
    updated: "now",
  },
  {
    id: "worker",
    name: "sync-worker",
    source: "github.com/novaflow/sync",
    service: "Worker",
    status: "Failed",
    branch: "release/1.8",
    updated: "18 min ago",
  },
];

const activity = [
  {
    id: "build",
    icon: Clock3,
    title: "Build started",
    detail: "dashboard-web from main",
    time: "Now",
    tone: "live",
  },
  {
    id: "deploy",
    icon: Check,
    title: "Deployment healthy",
    detail: "core-api is serving revision #184",
    time: "2 min",
    tone: "healthy",
  },
  {
    id: "failure",
    icon: CircleAlert,
    title: "Build failed",
    detail: "sync-worker has missing environment variable",
    time: "18 min",
    tone: "attention",
  },
];

const infrastructure = [
  {
    label: "CPU usage",
    value: "24%",
    meta: "8 cores allocated",
    icon: Cpu,
    health: "healthy" as Health,
  },
  {
    label: "Memory",
    value: "3.8 GB",
    meta: "of 16 GB",
    icon: HardDrive,
    health: "healthy" as Health,
  },
  {
    label: "Server",
    value: "fra-01",
    meta: "Hetzner / Germany",
    icon: Server,
    health: "healthy" as Health,
  },
];

const activeProjectId = shallowRef(projects[0].id);
const activeProject = computed(
  () => projects.find((project) => project.id === activeProjectId.value) ?? projects[0],
);
const liveProjectCount = computed(
  () => projects.filter((project) => project.status === "Live").length,
);
const attentionCount = computed(
  () => projects.filter((project) => project.status !== "Live").length,
);

function selectProject(id: string) {
  activeProjectId.value = id;
}

function badgeClass(status: DeploymentState) {
  return status === "Live"
    ? "bg-[#edf5ea] text-[#356b2e]"
    : status === "Building"
      ? "bg-[#fff0e6] text-[#a0491c]"
      : "bg-[#f5e8e4] text-[#8a4d3a]";
}

function activityClass(tone: string) {
  return tone === "healthy"
    ? "bg-[#edf5ea] text-[#397831]"
    : tone === "live"
      ? "bg-[#fff0e6] text-[#a5491b]"
      : "bg-[#f5e8e4] text-[#96543e]";
}
</script>

<template>
  <div class="grid gap-7 max-[600px]:gap-5">
    <header
      class="flex items-center justify-between gap-5 max-[600px]:items-start max-[600px]:flex-col"
    >
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Overview</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          One place to see workload health, deploy state, and next action.
        </p>
      </div>
      <div class="flex items-center gap-2 font-mono text-[10px] text-muted-foreground">
        <span class="status-dot" data-status="healthy" aria-hidden="true" />
        <span>Last sync 14:32 UTC</span>
      </div>
    </header>

    <section
      class="grid grid-cols-4 overflow-hidden rounded-lg border border-border max-[1100px]:grid-cols-2 max-[600px]:grid-cols-2"
      aria-label="Infrastructure summary"
    >
      <article
        v-for="(item, index) in [
          {
            label: 'Projects',
            value: projects.length,
            meta: `${liveProjectCount} live, ${attentionCount} need review`,
          },
          { label: 'Deployments', value: '12', meta: 'Last 24 hours' },
          { label: 'Healthy services', value: '5 / 6', meta: 'One build in progress' },
          { label: 'Action needed', value: '1', meta: 'Failed build needs config', signal: true },
        ]"
        :key="item.label"
        class="grid min-h-[132px] gap-2.5 border-r border-border bg-card p-5 last:border-r-0 max-[600px]:min-h-[112px] max-[600px]:p-4"
        :class="[
          item.signal ? 'bg-[#fffaf5] dark:bg-[#2a2725]' : '',
          index === 1 ? 'max-[1100px]:border-r-0' : '',
          index < 2 ? 'max-[1100px]:border-b' : '',
        ]"
      >
        <p class="ui-label">{{ item.label }}</p>
        <strong
          class="text-[30px] leading-none font-medium max-[600px]:text-[25px]"
          :class="item.signal ? 'text-[#b54513]' : ''"
          >{{ item.value }}</strong
        >
        <span class="text-xs text-muted-foreground">{{ item.meta }}</span>
      </article>
    </section>

    <section
      class="grid grid-cols-[minmax(0,1.55fr)_minmax(300px,0.8fr)] gap-5 max-[850px]:grid-cols-1"
    >
      <div class="min-w-0 rounded-lg border border-border bg-card p-[22px] max-[600px]:p-[17px]">
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="ui-label">Projects</p>
            <h2 class="mt-2 text-[17px] leading-none font-medium">Running workloads</h2>
          </div>
          <RouterLink
            class="inline-flex items-center gap-1 text-xs text-[#705a48] hover:text-foreground"
            to="/projects"
          >
            View all <ArrowUpRight :size="15" :stroke-width="1.5" />
          </RouterLink>
        </div>

        <div class="mt-5 -mr-2 -mb-2 -ml-2">
          <button
            v-for="project in projects"
            :key="project.id"
            class="grid w-full grid-cols-[auto_minmax(0,1fr)_auto_auto] items-center gap-3 rounded-[5px] bg-transparent px-2 py-3 text-left hover:bg-[#f3f1ef] dark:hover:bg-[#2a2725] max-[600px]:grid-cols-[auto_minmax(0,1fr)_auto]"
            :class="activeProjectId === project.id ? 'bg-[#f3f1ef] dark:bg-[#2a2725]' : ''"
            type="button"
            @click="selectProject(project.id)"
          >
            <span
              class="grid size-8 place-items-center rounded-[5px] bg-[#ece9e6] text-[#736b66] dark:bg-muted dark:text-muted-foreground"
              ><Box :size="17" :stroke-width="1.5"
            /></span>
            <span class="grid min-w-0 gap-1">
              <strong class="text-[13px] font-medium">{{ project.name }}</strong>
              <small class="max-w-[138px] truncate text-[11px] text-muted-foreground"
                >{{ project.service }} · {{ project.updated }}</small
              >
            </span>
            <span
              class="inline-flex items-center gap-1.5 rounded-[3px] px-[7px] py-1 font-mono text-[10px] max-[600px]:text-[9px]"
              :class="badgeClass(project.status)"
            >
              <span
                class="status-dot"
                :data-status="
                  project.status === 'Live'
                    ? 'healthy'
                    : project.status === 'Building'
                      ? 'live'
                      : 'inactive'
                "
                aria-hidden="true"
              />
              {{ project.status }}
            </span>
            <MoreHorizontal
              class="text-[#8e8782] max-[600px]:hidden"
              :size="18"
              :stroke-width="1.5"
            />
          </button>
        </div>
      </div>

      <aside
        class="rounded-lg border border-[#1d1a18] bg-[#1d1a18] p-[22px] text-[#eeeeee] max-[600px]:p-[17px]"
        aria-label="Selected project"
      >
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="ui-label !text-[#8a8380]">Selected project</p>
            <h2 class="mt-2 text-[17px] leading-none font-medium">{{ activeProject.name }}</h2>
          </div>
          <span
            class="rounded-[3px] bg-[#efedeb] px-[7px] py-1 font-mono text-[10px] text-[#5c5652]"
            >{{ activeProject.status }}</span
          >
        </div>
        <dl class="my-7 grid gap-4">
          <div class="grid gap-1">
            <dt class="font-mono text-[10px] uppercase text-[#8a8380]">Repository</dt>
            <dd class="m-0 break-all font-mono text-[11px] text-[#d9d4d0]">
              {{ activeProject.source }}
            </dd>
          </div>
          <div class="grid gap-1">
            <dt class="font-mono text-[10px] uppercase text-[#8a8380]">Branch</dt>
            <dd class="m-0 flex items-center gap-1 font-mono text-[11px] text-[#d9d4d0]">
              <GitBranch :size="14" :stroke-width="1.5" /> {{ activeProject.branch }}
            </dd>
          </div>
          <div class="grid gap-1">
            <dt class="font-mono text-[10px] uppercase text-[#8a8380]">Last update</dt>
            <dd class="m-0 font-mono text-[11px] text-[#d9d4d0]">{{ activeProject.updated }}</dd>
          </div>
        </dl>
        <div class="flex items-center gap-2">
          <Button size="sm" disabled title="Deployment controls arrive with project detail slice"
            >Open project</Button
          >
          <button
            class="rounded-[3px] bg-transparent px-2 py-1.5 text-xs text-[#b8b3b0] disabled:cursor-default"
            type="button"
            disabled
          >
            View logs
          </button>
        </div>
      </aside>
    </section>

    <section class="grid grid-cols-2 gap-5 max-[850px]:grid-cols-1">
      <div class="min-w-0 rounded-lg border border-border bg-card p-[22px] max-[600px]:p-[17px]">
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="ui-label">Activity</p>
            <h2 class="mt-2 text-[17px] leading-none font-medium">Recent changes</h2>
          </div>
          <button
            class="rounded-[3px] bg-transparent px-2 py-1.5 text-xs text-muted-foreground disabled:cursor-default"
            type="button"
            disabled
          >
            View activity
          </button>
        </div>
        <ol class="mt-5 grid list-none p-0">
          <li
            v-for="item in activity"
            :key="item.id"
            class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border-t border-border py-[13px]"
          >
            <span
              class="grid size-7 place-items-center rounded-full"
              :class="activityClass(item.tone)"
              ><component :is="item.icon" :size="15" :stroke-width="1.8"
            /></span>
            <span class="grid min-w-0 gap-1"
              ><strong class="text-[13px] font-medium">{{ item.title }}</strong
              ><small class="truncate text-[11px] text-muted-foreground">{{
                item.detail
              }}</small></span
            >
            <time class="font-mono text-[10px] text-muted-foreground">{{ item.time }}</time>
          </li>
        </ol>
      </div>

      <div class="min-w-0 rounded-lg border border-border bg-card p-[22px] max-[600px]:p-[17px]">
        <div class="flex items-center justify-between gap-4">
          <div>
            <p class="ui-label">Infrastructure</p>
            <h2 class="mt-2 text-[17px] leading-none font-medium">Server health</h2>
          </div>
          <span class="inline-flex items-center gap-1.5 text-[11px] text-[#4a7642]"
            ><span class="status-dot" data-status="healthy" /> Healthy</span
          >
        </div>
        <div class="mt-5 grid gap-px">
          <div
            v-for="item in infrastructure"
            :key="item.label"
            class="grid grid-cols-[auto_minmax(0,1fr)_auto] items-center gap-3 border-t border-border py-[11px] max-[600px]:grid-cols-[auto_minmax(0,1fr)]"
          >
            <span
              class="grid size-8 place-items-center rounded-[5px] bg-[#ece9e6] text-[#736b66] dark:bg-muted dark:text-muted-foreground"
              ><component :is="item.icon" :size="17" :stroke-width="1.5"
            /></span>
            <span class="grid gap-0.5"
              ><small class="truncate text-[11px] text-muted-foreground">{{ item.label }}</small
              ><strong class="text-[13px] font-medium">{{ item.value }}</strong></span
            >
            <span
              class="font-mono text-[10px] text-muted-foreground max-[600px]:col-start-2 max-[600px]:text-left"
              >{{ item.meta }}</span
            >
          </div>
        </div>
      </div>
    </section>
  </div>
</template>
