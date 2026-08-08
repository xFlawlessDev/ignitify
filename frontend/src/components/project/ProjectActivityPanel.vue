<script setup lang="ts">
import { Activity, CircleAlert, RefreshCw } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { ActivitySummary } from "@/lib/types";

defineProps<{
  activity: ActivitySummary[];
  error: string | null;
  loading: boolean;
}>();

defineEmits<{ retry: [] }>();

function formatTime(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}
</script>

<template>
  <section class="app-surface">
    <div class="app-panel-header flex items-center justify-between px-5 py-4">
      <div>
        <p class="ui-label">Audit trail</p>
        <h2 class="mt-2 text-base font-medium">Project activity</h2>
      </div>
      <Activity class="size-4 text-muted-foreground" :stroke-width="1.5" />
    </div>
    <div v-if="loading" class="divide-y divide-border" role="status" aria-label="Loading activity">
      <div
        v-for="index in 4"
        :key="index"
        class="grid gap-2 px-5 py-4 sm:grid-cols-[minmax(0,1fr)_auto]"
      >
        <div class="grid gap-2">
          <Skeleton class="h-3 w-40 max-w-full" />
          <Skeleton class="h-2.5 w-24 max-w-full" />
        </div>
        <Skeleton class="h-2.5 w-24 max-w-full" />
      </div>
    </div>
    <div
      v-else-if="error"
      class="flex items-start justify-between gap-4 px-5 py-8 max-[520px]:flex-col"
      role="alert"
    >
      <p class="flex items-center gap-2 text-sm text-destructive">
        <CircleAlert class="size-4" :stroke-width="1.5" />{{ error }}
      </p>
      <Button size="sm" variant="outline" @click="$emit('retry')">
        <RefreshCw class="size-4" :stroke-width="1.5" /> Retry
      </Button>
    </div>
    <div v-else-if="!activity.length" class="px-5 py-8 text-sm text-muted-foreground">
      No project activity yet.
    </div>
    <div v-else class="divide-y divide-border">
      <div
        v-for="item in activity"
        :key="item.id"
        class="grid gap-1 px-5 py-4 sm:grid-cols-[minmax(0,1fr)_auto]"
      >
        <div>
          <p class="text-sm font-medium">{{ item.action }}</p>
          <p class="mt-1 font-mono text-[11px] text-muted-foreground">
            {{ item.resource_type || "workspace"
            }}<span v-if="item.resource_id"> · {{ item.resource_id }}</span>
          </p>
        </div>
        <time class="font-mono text-[10px] text-muted-foreground" :datetime="item.created_at">
          {{ formatTime(item.created_at) }}
        </time>
      </div>
    </div>
  </section>
</template>
