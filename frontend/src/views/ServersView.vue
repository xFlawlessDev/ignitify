<script setup lang="ts">
import { RefreshCw, Server } from "@lucide/vue";
import { onMounted } from "vue";
import RuntimeStatusPanel from "@/components/runtime/RuntimeStatusPanel.vue";
import { Button } from "@/components/ui/button";
import { useRuntimeStatus } from "@/composables/useRuntimeStatus";

const { data, error, load, loading } = useRuntimeStatus();

onMounted(load);
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
    >
      <div>
        <p class="ui-label">Operations</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Servers</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Current control-plane host readiness and Docker capacity.
        </p>
      </div>
      <Button
        class="w-full shrink-0 sm:w-auto"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="load"
        ><RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Refresh</Button
      >
    </header>
    <section
      v-if="error"
      class="mt-4 border border-destructive/40 bg-card px-5 py-4 text-sm text-destructive"
      role="alert"
    >
      {{ error }}
    </section>
    <section class="mt-[22px] max-w-md">
      <RuntimeStatusPanel :runtime="data" :loading="loading" />
    </section>
    <p class="mt-4 flex items-center gap-2 text-xs text-muted-foreground">
      <Server class="size-4" :stroke-width="1.5" />Disk, per-container CPU, restart counts, and
      ingress health are not exposed by current runtime contract.
    </p>
  </div>
</template>
