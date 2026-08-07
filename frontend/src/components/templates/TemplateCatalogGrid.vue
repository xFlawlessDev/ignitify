<script setup lang="ts">
import { PackageSearch, RefreshCw } from "@lucide/vue";

import { Button } from "@/components/ui/button";
import Skeleton from "@/components/ui/skeleton/Skeleton.vue";
import type { TemplateMetadata } from "@/lib/template-catalog";
import TemplateCard from "./TemplateCard.vue";

defineProps<{
  templates: TemplateMetadata[];
  isLoading: boolean;
  error: string | null;
}>();

const emit = defineEmits<{
  select: [template: TemplateMetadata];
  retry: [];
}>();

const skeletons = Array.from({ length: 8 }, (_, index) => index);
</script>

<template>
  <div
    v-if="isLoading"
    class="grid gap-px border-l border-t border-border sm:grid-cols-2 xl:grid-cols-4"
  >
    <div
      v-for="skeleton in skeletons"
      :key="skeleton"
      class="min-h-64 border-r border-b border-border bg-card p-5"
    >
      <Skeleton class="size-12 rounded-md" />
      <Skeleton class="mt-8 h-5 w-2/3" />
      <Skeleton class="mt-3 h-4 w-full" />
      <Skeleton class="mt-2 h-4 w-4/5" />
      <div class="mt-8 flex gap-2">
        <Skeleton class="h-5 w-16" />
        <Skeleton class="h-5 w-20" />
      </div>
    </div>
  </div>

  <div
    v-else-if="error"
    class="border border-destructive/40 bg-destructive/5 px-6 py-12 text-center"
  >
    <PackageSearch class="mx-auto size-8 text-destructive" aria-hidden="true" />
    <h2 class="mt-4 text-base font-medium">Template catalog unavailable</h2>
    <p class="mt-2 text-sm text-muted-foreground">{{ error }}</p>
    <Button class="mt-6" variant="outline" type="button" @click="emit('retry')">
      <RefreshCw data-icon="inline-start" />
      Try again
    </Button>
  </div>

  <div
    v-else-if="templates.length === 0"
    class="border border-border bg-card px-6 py-16 text-center"
  >
    <PackageSearch class="mx-auto size-8 text-muted-foreground" aria-hidden="true" />
    <h2 class="mt-4 text-base font-medium">No templates found</h2>
    <p class="mt-2 text-sm text-muted-foreground">Try a different search or category.</p>
  </div>

  <div v-else class="grid gap-px border-l border-t border-border sm:grid-cols-2 xl:grid-cols-4">
    <TemplateCard
      v-for="template in templates"
      :key="template.id"
      :template="template"
      class="border-r border-b border-border"
      @select="emit('select', $event)"
    />
  </div>
</template>
