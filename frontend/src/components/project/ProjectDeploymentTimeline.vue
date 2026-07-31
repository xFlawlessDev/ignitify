<script setup lang="ts">
import { Check, CircleAlert, LoaderCircle } from "@lucide/vue";

export interface DeploymentItem {
  id: string;
  service: string;
  commit: string;
  actor: string;
  time: string;
  status: "success" | "building" | "failed";
}

defineProps<{ deployments: DeploymentItem[] }>();

function statusClasses(status: DeploymentItem["status"]) {
  return status === "success"
    ? "border-[var(--status-healthy)] text-[var(--status-healthy)]"
    : status === "building"
      ? "border-[var(--status-live)] text-[var(--status-live)]"
      : "border-destructive text-destructive";
}

function statusLabel(status: DeploymentItem["status"]) {
  return status === "success" ? "Ready" : status === "building" ? "In progress" : "Failed";
}
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-end justify-between gap-4 border-b border-border px-5 pt-5 pb-4">
      <div>
        <p class="ui-label">Delivery</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Deployment history</h2>
      </div>
      <span class="font-mono text-[11px] text-muted-foreground">Last 7 days</span>
    </div>

    <div class="px-5 py-1">
      <div
        v-for="(deployment, index) in deployments"
        :key="deployment.id"
        class="relative grid min-h-[68px] grid-cols-[28px_minmax(0,1fr)] items-center gap-3 sm:grid-cols-[28px_minmax(0,1fr)_auto]"
      >
        <span
          v-if="index < deployments.length - 1"
          class="absolute top-[42px] bottom-[-2px] left-[13px] w-px bg-border"
          aria-hidden="true"
        />
        <div
          class="z-1 grid size-7 place-items-center rounded-full border bg-card"
          :class="statusClasses(deployment.status)"
        >
          <Check v-if="deployment.status === 'success'" :size="13" :stroke-width="2" />
          <LoaderCircle v-else-if="deployment.status === 'building'" :size="13" :stroke-width="2" />
          <CircleAlert v-else :size="13" :stroke-width="2" />
        </div>
        <div class="grid min-w-0 gap-1">
          <div class="flex justify-between gap-3">
            <strong class="truncate text-[13px] font-medium">{{ deployment.service }}</strong>
            <span class="text-[11px] text-muted-foreground">{{ deployment.time }}</span>
          </div>
          <p class="m-0 text-[11px] text-muted-foreground">
            <code class="font-mono text-foreground">{{ deployment.commit }}</code> by
            {{ deployment.actor }}
          </p>
        </div>
        <span
          class="col-start-2 font-mono text-[10px] whitespace-nowrap sm:col-auto"
          :class="statusClasses(deployment.status).split(' ').at(-1)"
        >
          {{ statusLabel(deployment.status) }}
        </span>
      </div>
    </div>
  </section>
</template>
