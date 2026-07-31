<script setup lang="ts">
import type { Component } from "vue";

interface ActivityItem {
  title: string;
  detail: string;
  time: string;
  icon: Component;
  tone: "healthy" | "live" | "warning";
}

defineProps<{ items: ActivityItem[] }>();
</script>

<template>
  <section class="border border-border bg-card">
    <div class="border-b border-border px-5 py-4">
      <p class="ui-label">Activity</p>
      <h2 class="mt-2 text-base font-medium">System events</h2>
    </div>
    <div class="divide-y divide-border">
      <div v-for="item in items" :key="`${item.title}-${item.time}`" class="flex gap-3 px-5 py-4">
        <span
          class="mt-0.5 grid size-7 shrink-0 place-items-center rounded-sm"
          :class="
            item.tone === 'healthy'
              ? 'bg-[#eef5eb] text-[#47823e]'
              : item.tone === 'warning'
                ? 'bg-[#fdf0e8] text-[#d9500c]'
                : 'bg-muted text-foreground'
          "
        >
          <component :is="item.icon" class="size-3.5" stroke-width="1.5" />
        </span>
        <div class="min-w-0">
          <p class="text-sm leading-5">{{ item.title }}</p>
          <p class="mt-1 truncate text-xs text-muted-foreground">{{ item.detail }}</p>
          <p class="mt-2 font-mono text-[10px] uppercase text-muted-foreground/70">
            {{ item.time }}
          </p>
        </div>
      </div>
    </div>
  </section>
</template>
