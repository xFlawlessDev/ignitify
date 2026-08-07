<script setup lang="ts">
import {
  Box,
  FileCode2,
  GitBranch,
  LayoutGrid,
  List as ListIcon,
  Pencil,
  Plus,
  Rocket,
} from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { ServiceSummary } from "@/lib/types";

const props = withDefaults(
  defineProps<{
    canManage: boolean;
    error?: string | null;
    loading?: boolean;
    services: ServiceSummary[];
    projectVariableCount?: number;
    selectedServiceId?: string | null;
    view?: "list" | "catalog";
  }>(),
  { view: "catalog" },
);

const emit = defineEmits<{
  create: [];
  select: [service: ServiceSummary];
  edit: [service: ServiceSummary];
  retry: [];
  updateView: [view: "list" | "catalog"];
}>();

function sourceLabel(service: ServiceSummary) {
  if (service.source_config?.setup_required) return "setup required";
  if (service.source_config?.source === "application") {
    return `${service.source_config.builder ?? "application"} / ${service.source_config.repository ?? "repository"}`;
  }
  if (service.source_config?.source === "template") {
    return `template / ${service.source_config.template ?? "runtime"}`;
  }
  return service.kind === "compose"
    ? `compose / ${service.exposed_service}`
    : service.image_reference;
}
</script>

<template>
  <section :class="props.view === 'list' ? 'border border-border bg-card' : 'grid gap-3'">
    <div
      class="flex items-start justify-between gap-4 border-b border-border pt-5 pb-4 max-[520px]:flex-col"
      :class="props.view === 'list' ? 'px-5' : ''"
    >
      <div>
        <p class="ui-label">Deployment services</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Services</h2>
        <p class="mt-2 text-xs leading-5 text-muted-foreground">
          Each service has a deployment source and can override shared project environment keys.
        </p>
      </div>
      <div class="flex items-center gap-2 max-[520px]:w-full max-[520px]:justify-between">
        <div
          class="inline-flex items-center gap-0.5 rounded-sm border border-border bg-muted p-0.5"
          role="group"
          aria-label="Service view"
        >
          <button
            class="grid size-7 place-items-center rounded-[2px] transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            :class="
              props.view === 'list'
                ? 'bg-background text-foreground'
                : 'text-muted-foreground hover:bg-background/70 hover:text-foreground'
            "
            type="button"
            aria-label="Service list view"
            title="List view"
            :aria-pressed="props.view === 'list'"
            @click="emit('updateView', 'list')"
          >
            <ListIcon class="size-4" :stroke-width="1.5" />
          </button>
          <button
            class="grid size-7 place-items-center rounded-[2px] transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            :class="
              props.view === 'catalog'
                ? 'bg-background text-foreground'
                : 'text-muted-foreground hover:bg-background/70 hover:text-foreground'
            "
            type="button"
            aria-label="Service catalog view"
            title="Catalog view"
            :aria-pressed="props.view === 'catalog'"
            @click="emit('updateView', 'catalog')"
          >
            <LayoutGrid class="size-4" :stroke-width="1.5" />
          </button>
        </div>
        <Button v-if="props.canManage" size="sm" @click="emit('create')">
          <Plus class="size-4" :stroke-width="1.5" />
          Add service
        </Button>
      </div>
    </div>

    <div
      v-if="props.loading"
      class="divide-y divide-border"
      :class="props.view === 'catalog' ? 'border border-border bg-card' : ''"
      role="status"
      aria-label="Loading services"
    >
      <div v-for="index in 3" :key="index" class="flex min-h-[78px] items-center gap-3 px-5 py-3">
        <Skeleton class="size-[30px] shrink-0 rounded-[4px]" />
        <div class="grid flex-1 gap-2">
          <Skeleton class="h-3 w-36 max-w-full" />
          <Skeleton class="h-2.5 w-48 max-w-full" />
        </div>
        <Skeleton class="h-3 w-8" />
      </div>
    </div>
    <div
      v-else-if="props.error"
      class="px-5 py-5"
      :class="props.view === 'catalog' ? 'border border-destructive/40 bg-card' : ''"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ props.error }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
    </div>
    <div v-else-if="props.services.length && props.view === 'list'" class="divide-y divide-border">
      <div
        v-for="service in props.services"
        :key="service.id"
        class="grid min-h-[78px] grid-cols-[minmax(0,1fr)_auto] items-center gap-4 px-5 py-3 transition-colors hover:bg-muted/50 max-[420px]:gap-2"
        :class="props.selectedServiceId === service.id ? 'bg-muted/60' : ''"
      >
        <button
          class="flex min-w-0 items-center gap-3 text-left"
          type="button"
          @click="emit('select', service)"
        >
          <span
            class="grid size-[30px] shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
          >
            <FileCode2 v-if="service.kind === 'compose'" :size="15" :stroke-width="1.5" />
            <GitBranch
              v-else-if="service.source_config?.source === 'application'"
              :size="15"
              :stroke-width="1.5"
            />
            <Box v-else :size="15" :stroke-width="1.5" />
          </span>
          <div class="grid min-w-0 gap-1.5">
            <strong class="truncate text-[13px] font-medium">{{ service.name }}</strong>
            <code class="truncate font-mono text-[11px] text-muted-foreground">{{
              sourceLabel(service)
            }}</code>
            <span class="font-mono text-[10px] text-muted-foreground">
              {{ service.variables.length }} service key{{
                service.variables.length === 1 ? "" : "s"
              }}
              <template v-if="props.projectVariableCount">
                · {{ props.projectVariableCount }} inherited</template
              >
            </span>
          </div>
        </button>
        <div class="flex items-center gap-3">
          <Rocket
            v-if="service.desired_state === 'running'"
            class="size-3.5 text-[var(--status-healthy)]"
            :stroke-width="1.5"
            aria-label="Running"
          />
          <span class="font-mono text-[11px] text-muted-foreground"
            >g{{ service.desired_generation }}</span
          >
          <button
            v-if="props.canManage"
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
    <div v-else-if="props.services.length" class="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      <article
        v-for="service in props.services"
        :key="service.id"
        class="flex min-h-[190px] flex-col border border-border bg-card p-4"
        :class="props.selectedServiceId === service.id ? 'border-foreground/30' : ''"
      >
        <div class="flex items-start justify-between gap-3">
          <button
            class="flex min-w-0 flex-1 items-start gap-3 text-left focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            type="button"
            @click="emit('select', service)"
          >
            <span
              class="grid size-9 shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
            >
              <FileCode2 v-if="service.kind === 'compose'" :size="17" :stroke-width="1.5" />
              <GitBranch
                v-else-if="service.source_config?.source === 'application'"
                :size="17"
                :stroke-width="1.5"
              />
              <Box v-else :size="17" :stroke-width="1.5" />
            </span>
            <span class="grid min-w-0 gap-1.5 pt-0.5">
              <strong class="truncate text-[13px] font-medium">{{ service.name }}</strong>
              <code class="truncate font-mono text-[11px] text-muted-foreground">{{
                sourceLabel(service)
              }}</code>
            </span>
          </button>
          <button
            v-if="props.canManage"
            class="grid size-8 shrink-0 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
            type="button"
            :aria-label="`Edit ${service.name}`"
            title="Edit service"
            @click="emit('edit', service)"
          >
            <Pencil class="size-4" :stroke-width="1.5" />
          </button>
        </div>
        <div class="mt-auto grid gap-3 border-t border-border pt-3">
          <div class="flex items-center justify-between gap-3 text-[11px] text-muted-foreground">
            <span class="flex items-center gap-1.5">
              <Rocket
                v-if="service.desired_state === 'running'"
                class="size-3.5 text-[var(--status-healthy)]"
                :stroke-width="1.5"
              />
              {{ service.desired_state === "running" ? "Running" : "Stopped" }}
            </span>
            <span class="font-mono">g{{ service.desired_generation }}</span>
          </div>
          <span class="font-mono text-[10px] text-muted-foreground">
            {{ service.variables.length }} service key{{
              service.variables.length === 1 ? "" : "s"
            }}
            <template v-if="props.projectVariableCount">
              · {{ props.projectVariableCount }} inherited</template
            >
          </span>
        </div>
      </article>
    </div>
    <div
      v-else
      class="px-5 py-8"
      :class="props.view === 'catalog' ? 'border border-border bg-card' : ''"
    >
      <p class="text-sm font-medium">No services configured</p>
      <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
        Start with a container image or hardened Compose file. Git providers are prepared in the
        workspace and will be available after a provider connection is configured.
      </p>
    </div>
  </section>
</template>
