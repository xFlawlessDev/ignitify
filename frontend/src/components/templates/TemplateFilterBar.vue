<script setup lang="ts">
import { ListFilter, Search, X } from "@lucide/vue";

import { Button } from "@/components/ui/button";

defineProps<{
  query: string;
  activeTag: string;
  tags: string[];
  resultCount: number;
  totalCount: number;
}>();

const emit = defineEmits<{
  updateQuery: [value: string];
  updateTag: [value: string];
  clear: [];
}>();
</script>

<template>
  <div class="border-y border-border py-4">
    <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div class="flex min-w-0 flex-1 flex-col gap-3 sm:flex-row sm:items-center">
        <label class="relative block min-w-0 flex-1 sm:max-w-sm">
          <span class="sr-only">Search templates</span>
          <Search
            class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <input
            :value="query"
            type="search"
            placeholder="Search templates"
            class="h-10 w-full rounded-md border border-input bg-background pr-3 pl-10 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
            @input="emit('updateQuery', ($event.target as HTMLInputElement).value)"
          />
        </label>

        <label class="relative block sm:w-52">
          <span class="sr-only">Filter by category</span>
          <ListFilter
            class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <select
            :value="activeTag"
            class="h-10 w-full appearance-none rounded-md border border-input bg-background pr-8 pl-10 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
            @change="emit('updateTag', ($event.target as HTMLSelectElement).value)"
          >
            <option value="all">All categories</option>
            <option v-for="tag in tags" :key="tag" :value="tag">{{ tag }}</option>
          </select>
        </label>

        <Button
          v-if="query || activeTag !== 'all'"
          variant="ghost"
          size="sm"
          @click="emit('clear')"
        >
          <X data-icon="inline-start" />
          Clear
        </Button>
      </div>

      <p class="shrink-0 font-mono text-[11px] uppercase text-muted-foreground">
        {{ resultCount }} of {{ totalCount }} templates
      </p>
    </div>
  </div>
</template>
