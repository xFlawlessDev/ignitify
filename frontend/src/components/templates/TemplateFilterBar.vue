<script setup lang="ts">
import { ListFilter, Search, X } from "@lucide/vue";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

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

function updateTag(value: string | undefined) {
  emit("updateTag", value ?? "all");
}
</script>

<template>
  <div class="border-y border-border py-4">
    <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
      <div class="flex min-w-0 flex-1 flex-col gap-3 sm:flex-row sm:items-center">
        <Label class="relative block min-w-0 flex-1 sm:max-w-sm">
          <span class="sr-only">Search templates</span>
          <Search
            class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            :model-value="query"
            type="search"
            placeholder="Search templates"
            class="h-9 w-full rounded-[3px] border border-input bg-background pr-3 pl-10 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
            @input="emit('updateQuery', ($event.target as HTMLInputElement).value)"
          />
        </Label>

        <div class="relative block sm:w-52">
          <Label for="template-category-filter" class="sr-only">Filter by category</Label>
          <ListFilter
            class="pointer-events-none absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Select :model-value="activeTag" @update:model-value="updateTag">
            <SelectTrigger
              id="template-category-filter"
              class="h-9 w-full pr-8 pl-10 text-sm text-foreground"
              aria-label="Filter by category"
            >
              <SelectValue placeholder="All categories" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All categories</SelectItem>
              <SelectItem v-for="tag in tags" :key="tag" :value="tag">{{ tag }}</SelectItem>
            </SelectContent>
          </Select>
        </div>

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
