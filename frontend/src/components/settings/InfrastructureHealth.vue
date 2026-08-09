<script setup lang="ts">
import { Box, Database, GitBranch, Router } from "@lucide/vue";
import { computed } from "vue";

export interface InfrastructureHealth {
  database: "ready" | "unavailable";
  runtime: "ready" | "unavailable";
  worker: "ready" | "unavailable";
  ingress: "ready" | "unavailable";
}

const props = defineProps<{
  health: InfrastructureHealth | null;
}>();

const checks = computed(() => [
  { label: "Database", value: props.health?.database, icon: Database },
  { label: "Runtime", value: props.health?.runtime, icon: Box },
  { label: "Worker", value: props.health?.worker, icon: GitBranch },
  { label: "Traefik", value: props.health?.ingress, icon: Router },
]);
</script>

<template>
  <section class="app-surface" aria-labelledby="infrastructure-health-heading">
    <header class="app-panel-header px-5 py-4">
      <p class="ui-label">Infrastructure status</p>
      <h2 id="infrastructure-health-heading" class="mt-1.5 text-base font-medium">
        Control plane health
      </h2>
    </header>
    <dl class="grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0">
      <div v-for="check in checks" :key="check.label" class="flex items-center gap-3 px-5 py-3.5">
        <component
          :is="check.icon"
          class="size-4 shrink-0 text-muted-foreground"
          :stroke-width="1.5"
        />
        <div class="min-w-0">
          <dt class="text-xs font-medium">{{ check.label }}</dt>
          <dd
            class="mt-0.5 font-mono text-[11px]"
            :class="
              check.value === 'ready'
                ? 'text-metric-green'
                : check.value === 'unavailable'
                  ? 'text-destructive'
                  : 'text-muted-foreground'
            "
          >
            {{ check.value ?? "checking" }}
          </dd>
        </div>
      </div>
    </dl>
  </section>
</template>
