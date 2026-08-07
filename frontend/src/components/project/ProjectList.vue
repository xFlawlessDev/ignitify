<script setup lang="ts">
import { ArrowUpRight, Box, KeyRound } from "@lucide/vue";
import { RouterLink } from "vue-router";
import type { ProjectSummary } from "@/lib/types";

const props = withDefaults(
  defineProps<{
    projects: ProjectSummary[];
    view?: "list" | "catalog";
  }>(),
  { view: "catalog" },
);

function formatUpdatedAt(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}
</script>

<template>
  <section v-if="props.view === 'list'" class="border border-border bg-card" aria-label="Projects">
    <RouterLink
      v-for="project in props.projects"
      :key="project.id"
      class="grid min-h-[86px] grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3.5 border-b border-border px-4 py-3 text-foreground transition-colors last:border-b-0 hover:bg-muted sm:grid-cols-[32px_minmax(0,1fr)_auto_auto] sm:px-[18px]"
      :to="`/projects/${project.id}`"
    >
      <span
        class="grid size-[30px] place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
      >
        <Box :size="17" :stroke-width="1.5" />
      </span>
      <span class="grid min-w-0 gap-1.5">
        <strong class="truncate text-[13px] font-medium">{{ project.name }}</strong>
        <span
          class="flex min-w-0 items-center gap-1.5 truncate font-mono text-[11px] text-muted-foreground"
        >
          <KeyRound class="size-3 shrink-0" :stroke-width="1.5" />
          {{ project.default_environment.name }} environment
        </span>
      </span>
      <span class="hidden text-right sm:grid sm:gap-1">
        <span class="font-mono text-[10px] uppercase text-muted-foreground">{{
          project.role
        }}</span>
        <time class="font-mono text-[10px] text-muted-foreground" :datetime="project.updated_at">
          {{ formatUpdatedAt(project.updated_at) }}
        </time>
      </span>
      <ArrowUpRight class="text-muted-foreground" :size="16" :stroke-width="1.5" />
    </RouterLink>
  </section>
  <section v-else class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3" aria-label="Project catalog">
    <RouterLink
      v-for="project in props.projects"
      :key="project.id"
      class="flex min-h-[184px] flex-col justify-between border border-border bg-card p-4 text-foreground transition-colors hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
      :to="`/projects/${project.id}`"
    >
      <span class="flex items-start justify-between gap-3">
        <span
          class="grid size-9 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
        >
          <Box class="size-[17px]" :stroke-width="1.5" />
        </span>
        <ArrowUpRight class="text-muted-foreground" :size="16" :stroke-width="1.5" />
      </span>
      <span class="grid gap-2">
        <strong class="truncate text-[14px] font-medium">{{ project.name }}</strong>
        <span
          class="flex min-w-0 items-center gap-1.5 truncate font-mono text-[11px] text-muted-foreground"
        >
          <KeyRound class="size-3 shrink-0" :stroke-width="1.5" />
          {{ project.default_environment.name }} environment
        </span>
      </span>
      <span class="flex items-center justify-between gap-3 border-t border-border pt-3">
        <span class="font-mono text-[10px] uppercase text-muted-foreground">{{
          project.role
        }}</span>
        <time class="font-mono text-[10px] text-muted-foreground" :datetime="project.updated_at">
          {{ formatUpdatedAt(project.updated_at) }}
        </time>
      </span>
    </RouterLink>
  </section>
</template>
