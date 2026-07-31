<script setup lang="ts">
import type { Component } from "vue";

interface Props {
  label: string;
  value: string;
  icon: Component;
}

defineProps<Props>();
</script>

<template>
  <section class="grid min-h-36 content-between bg-background p-5">
    <div class="flex items-center justify-between gap-4">
      <p class="ui-label">{{ label }}</p>
      <component :is="icon" class="size-4 text-muted-foreground" stroke-width="1.5" />
    </div>
    <p class="text-4xl font-normal leading-none tracking-normal">{{ value }}</p>
  </section>
</template>
