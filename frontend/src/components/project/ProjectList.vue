<script setup lang="ts">
import { ArrowUpRight, Box } from "@lucide/vue";
import { RouterLink } from "vue-router";
import type { ProjectSummary } from "@/lib/types";

defineProps<{
  projects: ProjectSummary[];
}>();
</script>

<template>
  <section class="border border-border bg-card" aria-label="Projects">
    <RouterLink
      v-for="project in projects"
      :key="project.id"
      class="grid min-h-[78px] grid-cols-[32px_minmax(0,1fr)_auto] items-center gap-3.5 border-b border-border px-[18px] py-3 text-foreground last:border-b-0 hover:bg-muted"
      :to="`/projects/${project.id}`"
    >
      <span
        class="grid size-[30px] place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
      >
        <Box :size="17" :stroke-width="1.5" />
      </span>
      <span class="grid min-w-0 gap-1">
        <strong class="truncate text-[13px] font-medium">{{ project.name }}</strong>
        <span class="truncate text-[11px] text-muted-foreground">
          {{ project.default_environment.name }} environment
        </span>
      </span>
      <ArrowUpRight class="text-muted-foreground" :size="16" :stroke-width="1.5" />
    </RouterLink>
  </section>
</template>
