<script setup lang="ts">
import { computed } from "vue";
import type { MonitoringSample, TelemetryKey } from "@/composables/useSystemMonitoring";

interface ChartSeries {
  key: TelemetryKey;
  label: string;
  color: "signal" | "healthy" | "neutral" | "secondary";
}

interface Props {
  samples: MonitoringSample[];
  series: ChartSeries[];
  max?: number;
  unit: string;
}

const props = defineProps<Props>();
const width = 720;
const height = 228;
const padding = { top: 16, right: 12, bottom: 30, left: 12 };
const plotWidth = width - padding.left - padding.right;
const plotHeight = height - padding.top - padding.bottom;

const maxValue = computed(() => {
  if (props.max !== undefined) return props.max;
  const values = props.samples.flatMap((sample) =>
    props.series.map((series) => Number(sample[series.key])),
  );
  return Math.max(...values, 1) * 1.2;
});

const yTicks = computed(() => [0, 0.25, 0.5, 0.75, 1].map((step) => maxValue.value * step));

const lines = computed(() =>
  props.series.map((series) => ({
    ...series,
    points: props.samples
      .map((sample, index) => {
        const x = padding.left + (index / Math.max(props.samples.length - 1, 1)) * plotWidth;
        const y =
          padding.top + plotHeight - (Number(sample[series.key]) / maxValue.value) * plotHeight;
        return `${x},${y}`;
      })
      .join(" "),
  })),
);

const xLabels = computed(() =>
  props.samples.map((sample, index) => ({
    label: sample.time,
    x: padding.left + (index / Math.max(props.samples.length - 1, 1)) * plotWidth,
  })),
);

function formatTick(value: number) {
  return props.max === 100 ? `${Math.round(value)}%` : `${Math.round(value)} ${props.unit}`;
}
</script>

<template>
  <div class="trend-chart" role="img" aria-label="Resource usage trend chart">
    <div class="trend-chart__legend">
      <span v-for="item in series" :key="item.key" class="trend-chart__legend-item">
        <i :data-color="item.color" aria-hidden="true" />
        {{ item.label }}
      </span>
    </div>
    <svg class="trend-chart__svg" :viewBox="`0 0 ${width} ${height}`" preserveAspectRatio="none">
      <g class="trend-chart__grid" aria-hidden="true">
        <line
          v-for="(tick, index) in yTicks"
          :key="tick"
          :x1="padding.left"
          :x2="width - padding.right"
          :y1="padding.top + plotHeight - (index / 4) * plotHeight"
          :y2="padding.top + plotHeight - (index / 4) * plotHeight"
        />
      </g>
      <g class="trend-chart__ticks" aria-hidden="true">
        <text
          v-for="(tick, index) in yTicks"
          :key="tick"
          :x="padding.left"
          :y="padding.top + plotHeight - (index / 4) * plotHeight - 6"
        >
          {{ formatTick(tick) }}
        </text>
      </g>
      <polyline
        v-for="line in lines"
        :key="line.key"
        class="trend-chart__line"
        :data-color="line.color"
        :points="line.points"
        fill="none"
        vector-effect="non-scaling-stroke"
      />
      <g class="trend-chart__labels" aria-hidden="true">
        <text
          v-for="item in xLabels"
          :key="item.label"
          :x="item.x"
          :y="height - 7"
          text-anchor="middle"
        >
          {{ item.label }}
        </text>
      </g>
    </svg>
  </div>
</template>

<style scoped>
.trend-chart {
  min-width: 0;
}

.trend-chart__legend {
  display: flex;
  flex-wrap: wrap;
  gap: 14px;
  margin-bottom: 14px;
}

.trend-chart__legend-item {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.trend-chart__legend-item i {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--graphite-mid);
}

.trend-chart__legend-item i[data-color="signal"],
.trend-chart__line[data-color="signal"] {
  --chart-stroke: var(--status-live);
}

.trend-chart__legend-item i[data-color="healthy"],
.trend-chart__line[data-color="healthy"] {
  --chart-stroke: var(--status-healthy);
}

.trend-chart__legend-item i[data-color="neutral"],
.trend-chart__line[data-color="neutral"] {
  --chart-stroke: var(--muted-foreground);
}

.trend-chart__legend-item i[data-color="secondary"],
.trend-chart__line[data-color="secondary"] {
  --chart-stroke: var(--sidebar-accent);
}

.trend-chart__legend-item i[data-color="signal"] {
  background: var(--status-live);
}

.trend-chart__legend-item i[data-color="healthy"] {
  background: var(--status-healthy);
}

.trend-chart__legend-item i[data-color="neutral"] {
  background: var(--muted-foreground);
}

.trend-chart__legend-item i[data-color="secondary"] {
  background: var(--sidebar-accent);
}

.trend-chart__svg {
  display: block;
  width: 100%;
  height: auto;
  min-height: 190px;
  overflow: visible;
}

.trend-chart__grid line {
  stroke: var(--border);
  stroke-dasharray: 2 4;
  stroke-width: 1;
}

.trend-chart__ticks text,
.trend-chart__labels text {
  fill: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 9px;
}

.trend-chart__line {
  stroke: var(--chart-stroke);
  stroke-width: 1.7;
}
</style>
