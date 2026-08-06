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

const progressWidth = computed(() => `${Math.min(Math.max(props.progress ?? 0, 0), 100)}%`);
</script>

<template>
  <article class="metric-card" :data-tone="deltaTone">
    <div class="metric-card__heading">
      <div class="metric-card__label">
        <component :is="icon" class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <span>{{ label }}</span>
      </div>
      <span class="metric-card__delta">{{ delta }}</span>
    </div>

    <div class="metric-card__value">{{ value }}</div>
    <p class="metric-card__detail">{{ detail }}</p>

    <div v-if="progress !== undefined" class="metric-card__progress" aria-hidden="true">
      <span :style="{ width: progressWidth }" />
    </div>
    <svg
      v-else
      class="metric-card__sparkline"
      viewBox="0 0 100 32"
      preserveAspectRatio="none"
      aria-hidden="true"
    >
      <polyline :points="linePoints" fill="none" vector-effect="non-scaling-stroke" />
    </svg>
  </article>
</template>

<style scoped>
.metric-card {
  min-width: 0;
  border: 1px solid var(--border);
  background: var(--card);
  padding: 18px 18px 16px;
}

.metric-card__heading,
.metric-card__label {
  display: flex;
  align-items: center;
}

.metric-card__heading {
  justify-content: space-between;
  gap: 12px;
}

.metric-card__label {
  min-width: 0;
  gap: 8px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 11px;
  letter-spacing: 0;
  text-transform: uppercase;
}

.metric-card__label span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.metric-card__delta {
  flex: none;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.metric-card[data-tone="up"] .metric-card__delta {
  color: var(--status-live);
}

.metric-card[data-tone="down"] .metric-card__delta {
  color: var(--status-healthy);
}

.metric-card__value {
  margin-top: 22px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--foreground);
  font-family: var(--font-geist-mono);
  font-size: 1.5rem;
  line-height: 1;
}

.metric-card__detail {
  min-height: 32px;
  margin-top: 8px;
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.45;
}

.metric-card__progress,
.metric-card__sparkline {
  display: block;
  width: 100%;
  margin-top: 16px;
}

.metric-card__progress {
  height: 4px;
  overflow: hidden;
  background: var(--muted);
}

.metric-card__progress span {
  display: block;
  height: 100%;
  background: var(--status-live);
  transition: width 200ms ease;
}

.metric-card__sparkline {
  height: 32px;
  overflow: visible;
}

.metric-card__sparkline polyline {
  stroke: var(--status-live);
  stroke-width: 1.5;
}

.metric-card[data-tone="down"] .metric-card__sparkline polyline {
  stroke: var(--status-healthy);
}

@media (prefers-reduced-motion: reduce) {
  .metric-card__progress span {
    transition: none;
  }
}
</style>
