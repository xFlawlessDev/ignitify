<script setup lang="ts">
import { Layers3 } from "@lucide/vue";
import { Input } from "@/components/ui/input";

interface Props {
  concurrentBuilds: number;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (event: "update:concurrentBuilds", value: number): void;
}>();

function updateConcurrentBuilds(value: string | number) {
  const parsed = Number(value);
  emit("update:concurrentBuilds", Number.isFinite(parsed) ? Math.trunc(parsed) : 1);
}
</script>

<template>
  <section class="border border-border bg-card" aria-labelledby="build-heading">
    <header class="flex items-start gap-3 border-b border-border px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center border border-border bg-muted text-muted-foreground"
      >
        <Layers3 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Build capacity</p>
        <h2 id="build-heading" class="mt-1.5 text-base font-medium">Concurrent builds</h2>
        <p class="mt-1.5 max-w-[48ch] text-xs leading-5 text-muted-foreground">
          Configure how many deployments can build at the same time on each server.
        </p>
      </div>
    </header>

    <div class="grid gap-5 px-5 py-5">
      <div class="grid gap-2">
        <label for="concurrent-builds" class="text-xs font-medium">Builds per server</label>
        <div class="flex items-center gap-3">
          <Input
            id="concurrent-builds"
            class="max-w-[128px] rounded-[3px] text-center font-mono text-base tabular-nums"
            type="number"
            min="1"
            max="32"
            step="1"
            inputmode="numeric"
            :model-value="props.concurrentBuilds"
            aria-describedby="concurrent-builds-help"
            @update:model-value="updateConcurrentBuilds"
          />
          <span class="font-mono text-[11px] text-muted-foreground">simultaneous jobs</span>
        </div>
        <p id="concurrent-builds-help" class="text-[11px] leading-4 text-muted-foreground">
          Builds of the same service are always serialized, even when capacity is available.
        </p>
      </div>

      <div class="grid gap-3 border-t border-border pt-5 sm:grid-cols-2">
        <div>
          <p class="ui-label">Scope</p>
          <p class="mt-1.5 text-xs text-foreground">Applied independently per server</p>
        </div>
        <div>
          <p class="ui-label">Allowed range</p>
          <p class="mt-1.5 font-mono text-xs text-foreground">1–32 builds</p>
        </div>
      </div>
    </div>
  </section>
</template>
