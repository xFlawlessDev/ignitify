<script setup lang="ts">
import { CircleAlert, LoaderCircle, RotateCcw, Rocket, Square } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { DeploymentSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  deployments: DeploymentSummary[];
  error: string | null;
  loading: boolean;
  services: ServiceSummary[];
  submitting: boolean;
}>();

const emit = defineEmits<{
  deploy: [serviceId: string];
  stop: [serviceId: string];
  rollback: [deploymentId: string];
  retry: [];
}>();

function serviceName(serviceId: string) {
  return props.services.find((service) => service.id === serviceId)?.name ?? "Unknown service";
}

function isActive(status: DeploymentSummary["status"]) {
  return ["queued", "preparing", "running", "stopping"].includes(status);
}
</script>

<template>
  <section class="border border-border bg-card">
    <div
      class="flex items-end justify-between gap-4 border-b border-border px-5 pt-5 pb-4 max-[560px]:items-start max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Runtime</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Deployments</h2>
      </div>
      <div class="flex flex-wrap gap-2 max-[560px]:w-full max-[560px]:flex-col">
        <Button
          v-for="service in services"
          :key="service.id"
          class="w-full sm:w-auto"
          size="sm"
          :disabled="submitting"
          @click="emit('deploy', service.id)"
        >
          <Rocket class="size-4" :stroke-width="1.5" />
          Deploy {{ service.name }}
        </Button>
      </div>
    </div>

    <div
      v-if="loading"
      class="divide-y divide-border"
      role="status"
      aria-label="Loading deployments"
    >
      <div
        v-for="index in 4"
        :key="index"
        class="grid gap-3 px-5 py-4 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
      >
        <div class="grid min-w-0 gap-2">
          <Skeleton class="h-3 w-40 max-w-full" />
          <Skeleton class="h-2.5 w-12" />
        </div>
        <Skeleton class="h-2.5 w-20" />
      </div>
    </div>
    <section v-else-if="error && !deployments.length" class="px-5 py-5" role="alert">
      <p class="flex items-center gap-2 text-sm text-destructive">
        <CircleAlert class="size-4" :stroke-width="1.5" />
        {{ error }}
      </p>
      <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
    </section>
    <div v-else-if="deployments.length" class="divide-y divide-border">
      <section v-if="error" class="border-b border-border px-5 py-3" role="alert">
        <p class="flex items-center gap-2 text-sm text-destructive">
          <CircleAlert class="size-4" :stroke-width="1.5" />
          {{ error }}
        </p>
        <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
      </section>
      <div
        v-for="deployment in deployments"
        :key="deployment.id"
        class="grid gap-3 px-5 py-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
      >
        <div class="grid min-w-0 gap-1">
          <span class="truncate text-sm font-medium">{{ serviceName(deployment.service_id) }}</span>
          <span class="font-mono text-[11px] text-muted-foreground"
            >g{{ deployment.generation }}</span
          >
          <span v-if="deployment.failure_reason" class="break-words text-xs text-destructive">
            {{ deployment.failure_reason }}
          </span>
        </div>
        <div class="flex items-center justify-between gap-3 sm:justify-end">
          <span
            class="text-xs capitalize"
            :class="
              isActive(deployment.status) ? 'text-[var(--status-live)]' : 'text-muted-foreground'
            "
          >
            <LoaderCircle
              v-if="isActive(deployment.status)"
              class="mr-1 inline size-3 animate-spin"
              :stroke-width="1.5"
            />
            {{ deployment.status }}
          </span>
          <div class="flex items-center gap-1">
            <button
              v-if="deployment.status === 'healthy' || deployment.status === 'running'"
              class="grid size-8 place-items-center rounded-md border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
              type="button"
              :aria-label="`Stop ${serviceName(deployment.service_id)}`"
              title="Stop"
              :disabled="submitting"
              @click="emit('stop', deployment.service_id)"
            >
              <Square class="size-4" :stroke-width="1.5" />
            </button>
            <button
              v-if="deployment.status === 'healthy' || deployment.status === 'stopped'"
              class="grid size-8 place-items-center rounded-md border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
              type="button"
              :aria-label="`Rollback deployment ${deployment.generation}`"
              title="Rollback"
              :disabled="submitting"
              @click="emit('rollback', deployment.id)"
            >
              <RotateCcw class="size-4" :stroke-width="1.5" />
            </button>
          </div>
        </div>
      </div>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No deployments yet</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Deploy a configured service. Runtime state stays internal until managed domains land.
      </p>
    </div>
  </section>
</template>
