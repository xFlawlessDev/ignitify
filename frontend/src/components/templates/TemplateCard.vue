<script setup lang="ts">
import { ArrowUpRight, PackageOpen } from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";

import type { TemplateMetadata } from "@/lib/template-catalog";
import { templateFileUrl } from "@/lib/template-catalog";

const props = defineProps<{
  template: TemplateMetadata;
}>();

const emit = defineEmits<{
  select: [template: TemplateMetadata];
}>();

const imageFailed = shallowRef(false);
const logoUrl = computed(() =>
  props.template.logo ? templateFileUrl(props.template.id, props.template.logo) : "",
);
const visibleTags = computed(() => props.template.tags.slice(0, 3));

watch(
  () => props.template.id,
  () => {
    imageFailed.value = false;
  },
);
</script>

<template>
  <article class="group/template-card min-w-0">
    <Button
      variant="ghost"
      type="button"
      class="flex h-full w-full shrink flex-col items-stretch justify-start rounded-[10px] border border-border bg-card p-5 text-left whitespace-normal transition-[border-color,background-color] duration-200 hover:border-signal/70 hover:bg-muted/40 focus-visible:outline-2 focus-visible:outline-ring focus-visible:outline-offset-2"
      :aria-label="`View ${template.name} template`"
      @click="emit('select', template)"
    >
      <div class="flex min-h-12 min-w-0 items-start justify-between gap-4">
        <div
          class="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-[6px] border border-border bg-background p-2"
        >
          <img
            v-if="logoUrl && !imageFailed"
            :src="logoUrl"
            :alt="`${template.name} logo`"
            class="size-full object-contain"
            loading="lazy"
            @error="imageFailed = true"
          />
          <PackageOpen v-else class="size-6 text-signal" aria-hidden="true" />
        </div>
        <ArrowUpRight
          class="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-hover/template-card:-translate-y-0.5 group-hover/template-card:translate-x-0.5 group-hover/template-card:text-signal"
          aria-hidden="true"
        />
      </div>

      <div class="mt-6 min-w-0">
        <div class="flex items-baseline justify-between gap-3">
          <h2 class="min-w-0 flex-1 truncate text-base font-medium text-foreground">
            {{ template.name }}
          </h2>
          <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
            {{ template.version }}
          </span>
        </div>
        <p class="template-card-description mt-2 text-sm leading-6 text-muted-foreground">
          {{ template.description }}
        </p>
      </div>

      <div class="mt-auto flex flex-wrap gap-1.5 pt-6">
        <span
          v-for="tag in visibleTags"
          :key="tag"
          class="max-w-full break-words border border-border px-2 py-1 font-mono text-[10px] uppercase text-muted-foreground"
        >
          {{ tag }}
        </span>
      </div>
    </Button>
  </article>
</template>

<style scoped>
.template-card-description {
  display: -webkit-box;
  overflow: hidden;
  overflow-wrap: anywhere;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
}
</style>
