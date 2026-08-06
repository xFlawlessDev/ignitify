<script setup lang="ts">
import type { Component } from "vue";
import { computed } from "vue";

interface Props {
  label: string;
  value: string;
  detail: string;
  delta: string;
  deltaTone: "up" | "down" | "neutral";
  history: number[];
  icon: Component;
  progress?: number;
}

const props = defineProps<Props>();

const linePoints = computed(() => {
  if (!props.history.length) return "";
  const min = Math.min(...props.history);
  const max = Math.max(...props.history);
  const spread = max - min || 1;
  return props.history
    .map((value, index) => {
      const x = (index / Math.max(props.history.length - 1, 1)) * 96 + 2;
      const y = 29 - ((value - min) / spread) * 23;
      return `${x},${y}`;
    })
    .join(" ");
});

const progressValue = computed(() => Math.min(Math.max(props.progress ?? 0, 0), 100));

const deltaClass = computed(() => {
  if (props.deltaTone === "up") return "text-signal-orange";
  if (props.deltaTone === "down") return "text-metric-green";
  return "text-muted-foreground";
});
</script>

<template>
  <article class="min-w-0 border border-border bg-card px-[18px] pt-[18px] pb-4">
    <div class="flex items-center justify-between gap-3">
      <div
        class="flex min-w-0 items-center gap-2 font-mono text-[11px] text-muted-foreground uppercase"
      >
        <component :is="icon" class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <span class="truncate">{{ label }}</span>
      </div>
      <span class="shrink-0 font-mono text-[10px]" :class="deltaClass">{{ delta }}</span>
    </div>

    <div class="mt-[22px] truncate font-mono text-2xl leading-none text-foreground">
      {{ value }}
    </div>
    <p class="mt-2 min-h-8 text-[11px] leading-[1.45] text-muted-foreground">{{ detail }}</p>

    <svg
      v-if="progress !== undefined"
      class="mt-4 block h-1 w-full overflow-hidden"
      viewBox="0 0 100 1"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <rect width="100" height="1" class="fill-muted" />
      <rect :width="progressValue" height="1" class="fill-signal-orange" />
    </svg>
    <svg
      v-else
      class="mt-4 block h-8 w-full overflow-visible"
      viewBox="0 0 100 32"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <polyline
        :class="deltaTone === 'down' ? 'text-metric-green' : 'text-signal-orange'"
        :points="linePoints"
        fill="none"
        stroke="currentColor"
        stroke-width="1.5"
        vector-effect="non-scaling-stroke"
      />
    </svg>
  </article>
</template>
