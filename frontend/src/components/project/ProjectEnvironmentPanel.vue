<script setup lang="ts">
import { Copy, ExternalLink, Eye, EyeOff } from "@lucide/vue";
import { shallowRef } from "vue";

export interface EnvironmentVariable {
  key: string;
  value: string;
  secret: boolean;
}

defineProps<{ variables: EnvironmentVariable[] }>();

const showSecrets = shallowRef(false);
const copiedKey = shallowRef("");

function copyValue(variable: EnvironmentVariable) {
  copiedKey.value = variable.key;
  window.setTimeout(() => {
    copiedKey.value = "";
  }, 1800);
}
</script>

<template>
  <section class="border border-border bg-card">
    <div
      class="flex items-start justify-between gap-4 border-b border-border px-5 pt-5 pb-4 max-[480px]:flex-col"
    >
      <div>
        <p class="ui-label">Configuration</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Environment</h2>
      </div>
      <button
        class="inline-flex items-center gap-1.5 py-1 text-[11px] text-muted-foreground hover:text-foreground"
        type="button"
        @click="showSecrets = !showSecrets"
      >
        <EyeOff v-if="showSecrets" :size="14" :stroke-width="1.5" />
        <Eye v-else :size="14" :stroke-width="1.5" />
        {{ showSecrets ? "Hide values" : "Reveal values" }}
      </button>
    </div>

    <div class="px-5 py-1">
      <div
        v-for="variable in variables"
        :key="variable.key"
        class="grid min-h-[43px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_28px] items-center gap-3 border-b border-border last:border-b-0 max-[480px]:grid-cols-[minmax(0,1fr)_28px]"
      >
        <code class="truncate font-mono text-[11px] text-foreground">{{ variable.key }}</code>
        <span
          class="truncate font-mono text-[11px] text-muted-foreground max-[480px]:col-start-1 max-[480px]:col-end-3 max-[480px]:row-start-2 max-[480px]:pb-2"
          >{{ variable.secret && !showSecrets ? "••••••••••••" : variable.value }}</span
        >
        <button
          class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground hover:border-border hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="`Copy ${variable.key}`"
          @click="copyValue(variable)"
        >
          <Copy :size="14" :stroke-width="1.5" />
        </button>
      </div>
    </div>

    <div
      class="mt-3 flex items-center justify-between gap-3 border-t border-border px-5 pt-4 pb-[17px]"
    >
      <span class="grid gap-1.5">
        <span class="ui-label">Primary domain</span>
        <strong class="text-xs font-medium">app.novaflow.dev</strong>
      </span>
      <a
        class="text-muted-foreground hover:text-foreground"
        href="#"
        aria-label="Open primary domain"
      >
        <ExternalLink :size="15" :stroke-width="1.5" />
      </a>
    </div>

    <p
      v-if="copiedKey"
      class="border-t border-border bg-[color-mix(in_srgb,var(--status-live)_8%,transparent)] px-5 py-2.5 font-mono text-[11px] text-[var(--status-live)]"
      role="status"
    >
      {{ copiedKey }} copied
    </p>
  </section>
</template>
