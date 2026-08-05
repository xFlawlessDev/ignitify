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
  <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
    <div class="flex items-center justify-between gap-4">
      <p class="ui-label">{{ label }}</p>
      <component :is="icon" class="size-4 text-muted-foreground" stroke-width="1.5" />
    </div>
    <p class="text-3xl font-normal leading-none tracking-normal sm:text-4xl">{{ value }}</p>
  </section>
</template>
