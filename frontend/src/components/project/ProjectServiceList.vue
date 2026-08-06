<script setup lang="ts">
import { Box, FileCode2, Pencil, Plus } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { ServiceSummary } from "@/lib/types";

defineProps<{
  canManage: boolean;
  services: ServiceSummary[];
  projectVariableCount?: number;
}>();

const emit = defineEmits<{
  create: [];
  edit: [service: ServiceSummary];
}>();
</script>

<template>
  <section class="border border-border bg-card">
    <div
      class="flex items-start justify-between gap-4 border-b border-border px-5 pt-5 pb-4 max-[520px]:flex-col"
    >
      <div>
        <p class="ui-label">Deployment services</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Services</h2>
        <p class="mt-2 text-xs leading-5 text-muted-foreground">
          Each service has a deployment source and can override shared project environment keys.
        </p>
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
        class="grid min-h-[78px] grid-cols-[minmax(0,1fr)_auto] items-center gap-4 px-5 py-3 max-[420px]:gap-2"
      >
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-[30px] shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
          >
            <FileCode2 v-if="service.kind === 'compose'" :size="15" :stroke-width="1.5" />
            <Box v-else :size="15" :stroke-width="1.5" />
          </span>
          <div class="grid min-w-0 gap-1.5">
            <strong class="truncate text-[13px] font-medium">{{ service.name }}</strong>
            <code class="truncate font-mono text-[11px] text-muted-foreground">{{
              service.kind === "compose"
                ? `raw compose / ${service.exposed_service}`
                : service.image_reference
            }}</code>
            <span class="font-mono text-[10px] text-muted-foreground">
              {{ service.variables.length }} service key{{
                service.variables.length === 1 ? "" : "s"
              }}
              <template v-if="projectVariableCount">
                · {{ projectVariableCount }} inherited</template
              >
            </span>
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
      <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
        Start with a container image or hardened Compose file. Git providers are prepared in the
        workspace and will be available after a provider connection is configured.
      </p>
    </div>
  </section>
</template>
