<script setup lang="ts">
import { Pencil, Plus, Settings2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { ServiceSummary } from "@/lib/types";

defineProps<{
  canManage: boolean;
  services: ServiceSummary[];
}>();

const emit = defineEmits<{
  create: [];
  edit: [service: ServiceSummary];
}>();
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-end justify-between gap-4 border-b border-border px-5 pt-5 pb-4">
      <div>
        <p class="ui-label">Configuration</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Services</h2>
      </div>
      <Button v-if="canManage" size="sm" @click="emit('create')">
        <Plus class="size-4" :stroke-width="1.5" />
        Add service
      </Button>
    </div>

    <div v-if="services.length" class="divide-y divide-border">
      <div
        v-for="service in services"
        :key="service.id"
        class="grid min-h-[78px] grid-cols-[minmax(0,1fr)_auto] items-center gap-4 px-5 py-3"
      >
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-[30px] shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
          >
            <Settings2 :size="15" :stroke-width="1.5" />
          </span>
          <div class="grid min-w-0 gap-1">
            <strong class="truncate text-[13px] font-medium">{{ service.name }}</strong>
            <code class="truncate font-mono text-[11px] text-muted-foreground">{{
              service.kind === "compose"
                ? `compose / ${service.exposed_service}`
                : service.image_reference
            }}</code>
          </div>
        </div>
        <div class="flex items-center gap-3">
          <span class="font-mono text-[11px] text-muted-foreground"
            >g{{ service.desired_generation }}</span
          >
          <button
            v-if="canManage"
            class="grid size-8 place-items-center rounded-md border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
            type="button"
            :aria-label="`Edit ${service.name}`"
            title="Edit service"
            @click="emit('edit', service)"
          >
            <Pencil class="size-4" :stroke-width="1.5" />
          </button>
        </div>
      </div>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No services configured</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Add a digest-pinned image to define desired configuration.
      </p>
    </div>
  </section>
</template>
