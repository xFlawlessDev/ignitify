<script setup lang="ts">
import type { Component } from "vue";

interface Props {
  detail: string;
  label: string;
  value: string;
  icon: Component;
  tone?: "neutral" | "live" | "healthy" | "destructive";
}

const props = withDefaults(defineProps<Props>(), { tone: "neutral" });

function iconClass() {
  if (props.tone === "live") return "text-[var(--status-live)]";
  if (props.tone === "healthy") return "text-[var(--status-healthy)]";
  if (props.tone === "destructive") return "text-destructive";
  return "text-muted-foreground";
}
</script>

<template>
  <section class="grid min-h-36 content-between bg-background p-4 sm:min-h-40 sm:p-5">
    <div class="flex items-center justify-between gap-4">
      <p class="ui-label">{{ label }}</p>
      <component :is="icon" class="size-4" :class="iconClass()" stroke-width="1.5" />
    </div>
    <div>
      <p class="font-mono text-3xl leading-none tabular-nums sm:text-4xl">{{ value }}</p>
      <p class="mt-2 text-xs leading-4 text-muted-foreground">{{ detail }}</p>
    </div>
  </section>
</template>
