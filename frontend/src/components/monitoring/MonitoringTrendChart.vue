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

const colorClasses: Record<ChartSeries["color"], string> = {
  signal: "text-signal-orange",
  healthy: "text-metric-green",
  neutral: "text-muted-foreground",
  secondary: "text-[var(--sidebar-accent)]",
};

const legendColorClasses: Record<ChartSeries["color"], string> = {
  signal: "bg-signal-orange",
  healthy: "bg-metric-green",
  neutral: "bg-muted-foreground",
  secondary: "bg-[var(--sidebar-accent)]",
};

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
  <div class="min-w-0" role="img" aria-label="Resource usage trend chart">
    <div class="mb-3.5 flex flex-wrap gap-3.5">
      <span
        v-for="item in series"
        :key="item.key"
        class="inline-flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground"
      >
        <i
          class="size-[7px] shrink-0 rounded-full"
          :class="legendColorClasses[item.color]"
          aria-hidden="true"
        />
        {{ item.label }}
      </span>
    </div>
    <svg
      class="block h-auto min-h-[190px] w-full overflow-visible"
      :viewBox="`0 0 ${width} ${height}`"
      preserveAspectRatio="none"
    >
      <g class="text-border" aria-hidden="true">
        <line
          v-for="(tick, index) in yTicks"
          :key="tick"
          :x1="padding.left"
          :x2="width - padding.right"
          :y1="padding.top + plotHeight - (index / 4) * plotHeight"
          :y2="padding.top + plotHeight - (index / 4) * plotHeight"
          stroke="currentColor"
          stroke-dasharray="2 4"
          stroke-width="1"
        />
      </g>
      <g class="fill-muted-foreground font-mono text-[9px]" aria-hidden="true">
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
        class="stroke-current"
        :class="colorClasses[line.color]"
        :points="line.points"
        fill="none"
        stroke-width="1.7"
        vector-effect="non-scaling-stroke"
      />
      <g class="fill-muted-foreground font-mono text-[9px]" aria-hidden="true">
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
