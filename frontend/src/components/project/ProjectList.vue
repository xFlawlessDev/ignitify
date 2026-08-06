<script setup lang="ts">
import { ArrowUpRight, Box, KeyRound } from "@lucide/vue";
import { RouterLink } from "vue-router";
import type { ProjectSummary } from "@/lib/types";

defineProps<{
  projects: ProjectSummary[];
}>();

function formatUpdatedAt(value: string) {
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}
</script>

<template>
  <section class="border border-border bg-card" aria-label="Projects">
    <RouterLink
      v-for="project in projects"
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
</template>
