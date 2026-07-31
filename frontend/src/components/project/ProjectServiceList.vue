<script setup lang="ts">
import { MoreHorizontal, RefreshCw, Rocket } from "@lucide/vue";
import { shallowRef } from "vue";

export interface ProjectService {
  id: string;
  name: string;
  type: string;
  branch: string;
  status: "healthy" | "building" | "stopped";
  commit: string;
  deployedAt: string;
}

defineProps<{ services: ProjectService[] }>();

const actionMessage = shallowRef("");

function showAction(serviceName: string, action: string) {
  actionMessage.value = `${action} queued for ${serviceName}`;
  window.setTimeout(() => {
    actionMessage.value = "";
  }, 2400);
}

function statusLabel(status: ProjectService["status"]) {
  return status === "healthy" ? "Healthy" : status === "building" ? "Building" : "Stopped";
}

function statusDotClass(status: ProjectService["status"]) {
  return status === "healthy"
    ? "bg-[var(--status-healthy)]"
    : status === "building"
      ? "bg-[var(--status-live)]"
      : "bg-[var(--status-inactive)]";
}

function statusTextClass(status: ProjectService["status"]) {
  return status === "healthy"
    ? "text-[var(--status-healthy)]"
    : status === "building"
      ? "text-[var(--status-live)]"
      : "text-muted-foreground";
}
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-end justify-between gap-4 border-b border-border px-5 pt-5 pb-4">
      <div>
        <p class="ui-label">Runtime</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Services</h2>
      </div>
      <span class="font-mono text-[11px] text-muted-foreground"
        >{{ services.length }} services</span
      >
    </div>

    <div>
      <div
        v-for="service in services"
        :key="service.id"
        class="grid min-h-[72px] grid-cols-[minmax(0,1fr)_auto] items-center gap-3 border-b border-border px-5 py-3 last:border-b-0 md:grid-cols-[minmax(220px,1.3fr)_minmax(150px,0.8fr)_auto_auto] md:gap-5"
      >
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-[30px] shrink-0 place-items-center rounded-[4px] border border-border bg-muted"
          >
            <span class="size-1.5 rounded-full" :class="statusDotClass(service.status)" />
          </span>
          <div class="grid min-w-0 gap-1">
            <strong class="truncate text-[13px] font-medium">{{ service.name }}</strong>
            <span class="truncate text-[11px] text-muted-foreground"
              >{{ service.type }} · {{ service.branch }}</span
            >
          </div>
        </div>

        <div class="col-start-1 grid gap-1 pl-[41px] md:col-auto md:pl-0">
          <code class="font-mono text-[11px] text-foreground">{{ service.commit }}</code>
          <span class="text-[11px] text-muted-foreground">{{ service.deployedAt }}</span>
        </div>

        <div
          class="col-start-1 row-start-2 flex items-center gap-2 pl-[41px] text-[11px] whitespace-nowrap md:col-auto md:row-auto md:pl-0"
          :class="statusTextClass(service.status)"
        >
          <span class="size-1.5 rounded-full" :class="statusDotClass(service.status)" />
          {{ statusLabel(service.status) }}
        </div>

        <div
          class="col-start-2 row-span-2 row-start-1 flex items-center gap-0.5 md:col-auto md:row-auto"
        >
          <button
            class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
            type="button"
            :aria-label="`Deploy ${service.name}`"
            @click="showAction(service.name, 'Deploy')"
          >
            <Rocket :size="15" :stroke-width="1.5" />
          </button>
          <button
            class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
            type="button"
            :aria-label="`Restart ${service.name}`"
            @click="showAction(service.name, 'Restart')"
          >
            <RefreshCw :size="15" :stroke-width="1.5" />
          </button>
          <button
            class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
            type="button"
            :aria-label="`Open ${service.name} actions`"
            @click="showAction(service.name, 'More actions')"
          >
            <MoreHorizontal :size="16" :stroke-width="1.5" />
          </button>
        </div>
      </div>
    </div>

    <p
      v-if="actionMessage"
      class="border-t border-border bg-[color-mix(in_srgb,var(--status-live)_8%,transparent)] px-5 py-2.5 font-mono text-[11px] text-[var(--status-live)]"
      role="status"
    >
      {{ actionMessage }}
    </p>
  </section>
</template>
