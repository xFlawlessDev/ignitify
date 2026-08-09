<script setup lang="ts">
import { Cpu } from "@lucide/vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

defineProps<{
  concurrentBuilds: number;
  error?: string;
}>();

const emit = defineEmits<{
  (event: "update:concurrentBuilds", value: number): void;
}>();

function update(value: string | number) {
  const parsed = Number(value);
  emit("update:concurrentBuilds", Number.isFinite(parsed) ? parsed : 0);
}
</script>

<template>
  <section class="app-surface" aria-labelledby="build-capacity-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <Cpu class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Build capacity</p>
        <h2 id="build-capacity-heading" class="mt-1.5 text-base font-medium">Host build slots</h2>
        <p class="mt-1.5 max-w-[58ch] text-xs leading-5 text-muted-foreground">
          Limit how many application builds can run on this control-plane host at one time. New
          builds wait in the deployment queue until a slot is available.
        </p>
      </div>
    </header>

    <div class="grid gap-2 border-t border-border px-5 py-4 sm:max-w-xs">
      <Label for="concurrent-builds" class="text-xs font-medium">Concurrent builds</Label>
      <Input
        id="concurrent-builds"
        :model-value="concurrentBuilds"
        class="rounded-[3px] font-mono"
        type="number"
        min="1"
        max="32"
        step="1"
        inputmode="numeric"
        :aria-invalid="Boolean(error)"
        @update:model-value="update"
      />
      <p v-if="error" class="text-[11px] text-destructive" role="alert">{{ error }}</p>
      <p v-else class="text-[11px] text-muted-foreground">Allowed range: 1 to 32.</p>
    </div>
  </section>
</template>
