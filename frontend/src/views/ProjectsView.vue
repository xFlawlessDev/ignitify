<script setup lang="ts">
import { ArrowUpRight, Box, GitBranch, MoreHorizontal, Plus } from "@lucide/vue";
import { RouterLink } from "vue-router";
import { Button } from "@/components/ui/button";

const projects = [
  {
    id: "nova-api",
    name: "Nova API",
    description: "Core API and background jobs",
    repository: "novaflow/api",
    services: 3,
    status: "Live",
    updatedAt: "12 min ago",
  },
  {
    id: "docs",
    name: "Docs",
    description: "Public documentation site",
    repository: "novaflow/docs",
    services: 1,
    status: "Live",
    updatedAt: "2 hours ago",
  },
  {
    id: "staging",
    name: "Staging",
    description: "Pre-production verification environment",
    repository: "novaflow/api",
    services: 3,
    status: "Building",
    updatedAt: "Building now",
  },
];
</script>

<template>
  <div class="max-w-[1160px]">
    <header
      class="flex items-end justify-between gap-5 border-b border-border pb-[25px] max-[560px]:items-start max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Workspace</p>
        <h1 class="mt-3 text-[30px] leading-none font-normal">Projects</h1>
        <p class="mt-2 text-xs text-muted-foreground">
          Deployments grouped by product and environment.
        </p>
      </div>
      <Button
        class="max-[560px]:w-full"
        disabled
        title="Project creation arrives with deployment domain slice"
      >
        <Plus class="size-4" stroke-width="1.5" />
        New project
      </Button>
    </header>

    <section class="mt-[22px] border border-border bg-card" aria-label="Projects">
      <RouterLink
        v-for="project in projects"
        :key="project.id"
        class="grid min-h-[78px] grid-cols-[32px_minmax(180px,1.4fr)_minmax(150px,1fr)_80px_80px_100px_28px] items-center gap-3.5 border-b border-border px-[18px] py-3 text-foreground last:border-b-0 hover:bg-muted max-[900px]:grid-cols-[32px_minmax(160px,1fr)_80px_80px_28px] max-[560px]:grid-cols-[32px_minmax(0,1fr)_28px] max-[560px]:gap-2.5"
        :to="`/projects/${project.id}`"
      >
        <span
          class="grid size-[30px] place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
          ><Box :size="17" :stroke-width="1.5"
        /></span>
        <span class="grid min-w-0 gap-1">
          <strong class="text-[13px] font-medium">{{ project.name }}</strong>
          <span class="truncate text-[11px] text-muted-foreground">{{ project.description }}</span>
        </span>
        <span
          class="flex items-center gap-1.5 truncate text-[11px] text-muted-foreground max-[900px]:hidden"
          ><GitBranch :size="14" :stroke-width="1.5" /> {{ project.repository }}</span
        >
        <span class="truncate text-[11px] text-muted-foreground max-[560px]:hidden"
          >{{ project.services }} {{ project.services === 1 ? "service" : "services" }}</span
        >
        <span
          class="flex items-center gap-1.5 font-mono text-[10px] uppercase"
          :class="
            project.status === 'Live' ? 'text-[var(--status-healthy)]' : 'text-[var(--status-live)]'
          "
        >
          <span class="status-dot" :data-status="project.status === 'Live' ? 'healthy' : 'live'" />
          {{ project.status }}
        </span>
        <span class="truncate text-[11px] text-muted-foreground max-[900px]:hidden">{{
          project.updatedAt
        }}</span>
        <MoreHorizontal
          class="text-muted-foreground max-[560px]:hidden"
          :size="17"
          :stroke-width="1.5"
        />
        <ArrowUpRight
          class="hidden text-muted-foreground max-[560px]:block"
          :size="16"
          :stroke-width="1.5"
        />
      </RouterLink>
    </section>
  </div>
</template>
