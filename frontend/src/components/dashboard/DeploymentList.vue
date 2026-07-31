<script setup lang="ts">
import { ArrowUpRight, GitBranch, MoreHorizontal } from "@lucide/vue";

interface Deployment {
  name: string;
  project: string;
  branch: string;
  status: "live" | "building" | "failed";
  commit: string;
  updated: string;
}

defineProps<{ deployments: Deployment[] }>();

const statusLabel = {
  live: "Live",
  building: "Building",
  failed: "Failed",
};
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-center justify-between border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Deployments</p>
        <h2 class="mt-2 text-base font-medium">Recent releases</h2>
      </div>
      <button
        type="button"
        class="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
      >
        View all
        <ArrowUpRight class="ml-1 inline size-3.5" stroke-width="1.5" />
      </button>
    </div>

    <div class="divide-y divide-border">
      <div
        v-for="deployment in deployments"
        :key="deployment.name"
        class="grid gap-3 px-5 py-4 sm:grid-cols-[minmax(0,1.4fr)_minmax(0,1fr)_auto_auto] sm:items-center"
      >
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-sm bg-muted text-muted-foreground"
          >
            <GitBranch class="size-4" stroke-width="1.5" />
          </span>
          <div class="min-w-0">
            <p class="truncate text-sm font-medium">{{ deployment.name }}</p>
            <p class="mt-1 truncate text-xs text-muted-foreground">{{ deployment.project }}</p>
          </div>
        </div>
        <div class="min-w-0 text-xs text-muted-foreground">
          <p class="truncate">{{ deployment.branch }}</p>
          <p class="mt-1 font-mono text-[10px] text-muted-foreground/70">{{ deployment.commit }}</p>
        </div>
        <div
          class="flex items-center gap-2 text-xs"
          :class="
            deployment.status === 'failed'
              ? 'text-destructive'
              : deployment.status === 'building'
                ? 'text-[#d9500c]'
                : 'text-[#47823e]'
          "
        >
          <span
            class="status-dot"
            :data-status="deployment.status === 'live' ? 'healthy' : 'live'"
            aria-hidden="true"
          ></span>
          {{ statusLabel[deployment.status] }}
        </div>
        <div class="flex items-center justify-between gap-4 sm:justify-end">
          <span class="text-xs text-muted-foreground">{{ deployment.updated }}</span>
          <button
            type="button"
            class="grid size-7 place-items-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
            :aria-label="`Open ${deployment.name} actions`"
          >
            <MoreHorizontal class="size-4" stroke-width="1.5" />
          </button>
        </div>
      </div>
    </div>
  </section>
</template>
