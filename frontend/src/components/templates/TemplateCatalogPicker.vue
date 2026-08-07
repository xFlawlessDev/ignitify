<script setup lang="ts">
import { computed, nextTick, onUnmounted, shallowRef, watch } from "vue";
import { Boxes, ChevronLeft, ChevronRight } from "@lucide/vue";
import TemplateCatalogGrid from "@/components/templates/TemplateCatalogGrid.vue";
import TemplateDetailDialog from "@/components/templates/TemplateDetailDialog.vue";
import TemplateFilterBar from "@/components/templates/TemplateFilterBar.vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogDescription,
  DialogHeader,
  DialogScrollContent,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { useTemplateCatalog } from "@/composables/useTemplateCatalog";
import type { TemplateApplication, TemplateMetadata } from "@/lib/template-catalog";

const emit = defineEmits<{
  apply: [application: TemplateApplication];
}>();

const query = shallowRef("");
const activeTag = shallowRef("all");
const currentPage = shallowRef(1);
const pageSize = 24;
const selectedTemplate = shallowRef<TemplateMetadata | null>(null);
const isCatalogOpen = shallowRef(false);
const isDetailOpen = shallowRef(false);
const { templates, pagination, isLoading, error, loadTemplates } = useTemplateCatalog();

const tags = computed(() => {
  const uniqueTags = new Set(templates.value.flatMap((template) => template.tags));
  return [...uniqueTags].sort((left, right) => left.localeCompare(right));
});
let filterTimer: ReturnType<typeof setTimeout> | undefined;

function loadCatalogPage() {
  void loadTemplates({
    page: currentPage.value,
    pageSize,
    query: query.value,
    tag: activeTag.value,
  });
}

function goToPage(page: number) {
  const nextPage = Math.min(Math.max(1, page), pagination.value.totalPages || 1);
  if (nextPage === currentPage.value) return;
  currentPage.value = nextPage;
  loadCatalogPage();
}

function selectTemplate(template: TemplateMetadata) {
  selectedTemplate.value = template;
  isCatalogOpen.value = false;
  void nextTick(() => {
    isDetailOpen.value = true;
  });
}

function applyTemplate(application: TemplateApplication) {
  emit("apply", application);
}

function reopenCatalog() {
  void nextTick(() => {
    isCatalogOpen.value = true;
  });
}

function clearFilters() {
  query.value = "";
  activeTag.value = "all";
}

watch(isCatalogOpen, (open) => {
  if (open) loadCatalogPage();
});

watch([query, activeTag], () => {
  if (!isCatalogOpen.value) return;
  currentPage.value = 1;
  if (filterTimer) clearTimeout(filterTimer);
  filterTimer = setTimeout(loadCatalogPage, 300);
});

watch(
  () => pagination.value.page,
  (page) => {
    currentPage.value = page;
  },
);

onUnmounted(() => {
  if (filterTimer) clearTimeout(filterTimer);
});
</script>

<template>
  <Dialog v-model:open="isCatalogOpen">
    <DialogTrigger as-child>
      <Button type="button" variant="outline">
        <Boxes data-icon="inline-start" :stroke-width="1.5" />
        Choose template
      </Button>
    </DialogTrigger>
    <DialogScrollContent
      class="max-h-[min(42rem,calc(100vh-2rem))] max-w-4xl grid-rows-[auto_minmax(0,1fr)] gap-0 overflow-hidden p-0"
    >
      <DialogHeader class="border-b border-border px-5 py-4 pr-12">
        <DialogTitle>Choose a template</DialogTitle>
        <DialogDescription>
          Select a starting Compose configuration for this service.
        </DialogDescription>
      </DialogHeader>
      <div class="min-h-0 overflow-y-auto px-5 pb-5" aria-live="polite">
        <TemplateFilterBar
          :query="query"
          :active-tag="activeTag"
          :tags="tags"
          :result-count="templates.length"
          :total-count="pagination.total"
          @update-query="query = $event"
          @update-tag="activeTag = $event"
          @clear="clearFilters"
        />
        <div class="mt-4">
          <TemplateCatalogGrid
            :templates="templates"
            :is-loading="isLoading"
            :error="error"
            @select="selectTemplate"
            @retry="loadCatalogPage"
          />
        </div>
        <nav
          v-if="pagination.totalPages > 1"
          class="mt-4 flex items-center justify-between gap-4 border border-border bg-card px-4 py-3 max-[640px]:items-start max-[640px]:flex-col"
          aria-label="Template pagination"
        >
          <p class="text-xs text-muted-foreground" aria-live="polite">
            Page {{ pagination.page }} of {{ pagination.totalPages }}
          </p>
          <div class="flex items-center gap-2">
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="!pagination.hasPreviousPage || isLoading"
              aria-label="Previous page"
              @click="goToPage(currentPage - 1)"
            >
              <ChevronLeft class="size-4" :stroke-width="1.5" />
            </Button>
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="!pagination.hasNextPage || isLoading"
              aria-label="Next page"
              @click="goToPage(currentPage + 1)"
            >
              <ChevronRight class="size-4" :stroke-width="1.5" />
            </Button>
          </div>
        </nav>
      </div>
    </DialogScrollContent>
  </Dialog>
  <TemplateDetailDialog
    v-model:open="isDetailOpen"
    :template="selectedTemplate"
    @apply="applyTemplate"
    @back="reopenCatalog"
  />
</template>
