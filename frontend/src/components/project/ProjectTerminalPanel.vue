<script setup lang="ts">
import { CircleAlert, Terminal } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { TerminalCapability } from "@/lib/types";

defineProps<{
  capability: TerminalCapability | null;
  error: string | null;
  loading: boolean;
}>();

defineEmits<{ retry: [] }>();
</script>

<template>
  <section class="mt-[22px] border border-border bg-card">
    <div class="flex items-center justify-between border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Runtime access</p>
        <h2 class="mt-2 text-base font-medium">Terminal</h2>
      </div>
      <Terminal class="size-4 text-muted-foreground" :stroke-width="1.5" />
    </div>
    <div v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" role="status">
      Checking terminal capability...
    </div>
    <div v-else-if="error" class="px-5 py-8 text-sm text-destructive" role="alert">
      <p>{{ error }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="$emit('retry')">Retry</Button>
    </div>
    <div v-else class="px-5 py-8">
      <p class="flex items-center gap-2 text-sm font-medium">
        <CircleAlert class="size-4 text-muted-foreground" :stroke-width="1.5" />{{
          capability?.available ? "Terminal ready" : "Terminal unavailable"
        }}
      </p>
      <p class="mt-2 text-xs leading-5 text-muted-foreground">
        {{ capability?.reason ?? "Capability has not been reported." }}
      </p>
    </div>
  </section>
</template>
