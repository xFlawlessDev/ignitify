<script setup lang="ts">
import { ArrowUpRight, Box, RefreshCw } from "@lucide/vue";
import { computed, onMounted } from "vue";
import { RouterLink } from "vue-router";
import { Button } from "@/components/ui/button";
import { useProjects } from "@/composables/useProjects";

const { data, error, load, loading } = useProjects();
const projectCount = computed(() => data.value.length);

onMounted(load);
</script>

<template>
  <div class="max-w-[1160px]">
    <header class="border-b border-border pb-[25px]">
      <p class="ui-label">Control plane</p>
      <h1 class="mt-2.5 text-[30px] leading-none font-medium">Overview</h1>
      <p class="mt-2.5 text-[13px] text-muted-foreground">Workspace projects and configuration.</p>
    </header>

    <p
      v-if="loading"
      class="mt-[22px] border border-border bg-card px-5 py-8 text-sm text-muted-foreground"
      role="status"
    >
      Loading workspace...
    </p>
    <section
      v-else-if="error"
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-8"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button class="mt-4" variant="outline" size="sm" @click="load">
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Retry
      </Button>
    </section>
    <section
      v-else
      class="mt-[22px] grid border border-border bg-card sm:grid-cols-[minmax(0,1fr)_auto]"
    >
      <div class="flex items-center gap-4 p-5">
        <span
          class="grid size-10 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
        >
          <Box :size="19" :stroke-width="1.5" />
        </span>
        <div>
          <p class="ui-label">Projects</p>
          <strong class="mt-1 block text-[28px] leading-none font-medium">{{
            projectCount
          }}</strong>
        </div>
      </div>
      <RouterLink
        class="flex items-center gap-1 border-t border-border px-5 text-xs text-muted-foreground hover:text-foreground sm:border-t-0 sm:border-l"
        to="/projects"
      >
        {{ projectCount === 0 ? "Create project" : "View projects" }}
        <ArrowUpRight :size="15" :stroke-width="1.5" />
      </RouterLink>
    </section>
  </div>
</template>
