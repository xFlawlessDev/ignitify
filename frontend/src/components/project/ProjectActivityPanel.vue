<script setup lang="ts">
import { Activity, ChevronLeft, ChevronRight, CircleAlert, RefreshCw } from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type { ActivitySummary } from "@/lib/types";

const props = defineProps<{
  activity: ActivitySummary[];
  error: string | null;
  loading: boolean;
}>();

defineEmits<{ retry: [] }>();

const ACTIVITY_PER_PAGE = 10;
const { t } = useI18n();
const currentPage = shallowRef(1);
const activityCount = computed(() => props.activity.length);
const pageCount = computed(() => Math.max(1, Math.ceil(activityCount.value / ACTIVITY_PER_PAGE)));
const visibleActivity = computed(() => {
  const start = (currentPage.value - 1) * ACTIVITY_PER_PAGE;
  return props.activity.slice(start, start + ACTIVITY_PER_PAGE);
});
const firstVisibleActivity = computed(() =>
  activityCount.value === 0 ? 0 : (currentPage.value - 1) * ACTIVITY_PER_PAGE + 1,
);
const lastVisibleActivity = computed(() =>
  Math.min(currentPage.value * ACTIVITY_PER_PAGE, activityCount.value),
);

watch(
  pageCount,
  (count) => {
    if (currentPage.value > count) currentPage.value = count;
  },
  { immediate: true },
);

watch(
  () => props.activity,
  () => {
    currentPage.value = 1;
  },
);

function goToPreviousPage() {
  currentPage.value = Math.max(1, currentPage.value - 1);
}

function goToNextPage() {
  currentPage.value = Math.min(pageCount.value, currentPage.value + 1);
}

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
        v-for="item in visibleActivity"
        :key="item.id"
        class="grid gap-1 px-5 py-4 sm:grid-cols-[minmax(0,1fr)_auto]"
      >
        <div>
          <p class="text-sm font-medium">{{ item.action }}</p>
          <p class="mt-1 font-mono text-[11px] text-muted-foreground">
            {{ item.resource_type || "workspace"
            }}<span v-if="item.resource_id"> · {{ item.resource_id }}</span>
          </p>
          <p
            v-if="item.correlation_id"
            class="mt-1 truncate font-mono text-[10px] text-muted-foreground"
            :title="item.correlation_id"
          >
            {{ t("activity.correlationId") }} · {{ item.correlation_id }}
          </p>
        </div>
        <time class="font-mono text-[10px] text-muted-foreground" :datetime="item.created_at">
          {{ formatTime(item.created_at) }}
        </time>
      </div>
    </div>
    <nav
      v-if="pageCount > 1"
      class="flex items-center justify-between gap-4 border-t border-border px-5 py-3 max-[560px]:items-start max-[560px]:flex-col"
      aria-label="Project activity pagination"
    >
      <p class="text-xs text-muted-foreground" aria-live="polite">
        Showing {{ firstVisibleActivity }}–{{ lastVisibleActivity }} of {{ activityCount }} activity
        entries
      </p>
      <div class="flex items-center gap-2">
        <Button
          size="icon-sm"
          variant="outline"
          :disabled="currentPage === 1"
          aria-label="Previous activity page"
          @click="goToPreviousPage"
        >
          <ChevronLeft class="size-4" :stroke-width="1.5" />
        </Button>
        <span class="min-w-20 text-center font-mono text-xs text-muted-foreground">
          Page {{ currentPage }} of {{ pageCount }}
        </span>
        <Button
          size="icon-sm"
          variant="outline"
          :disabled="currentPage === pageCount"
          aria-label="Next activity page"
          @click="goToNextPage"
        >
          <ChevronRight class="size-4" :stroke-width="1.5" />
        </Button>
      </div>
    </nav>
  </section>
</template>
