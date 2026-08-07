<script setup lang="ts">
import { computed, onUnmounted, shallowRef, watch } from "vue";
import { ChevronLeft, ChevronRight, PackageOpen } from "@lucide/vue";

import TemplateCatalogGrid from "@/components/templates/TemplateCatalogGrid.vue";
import TemplateDetailDialog from "@/components/templates/TemplateDetailDialog.vue";
import TemplateFilterBar from "@/components/templates/TemplateFilterBar.vue";
import { useTemplateCatalog } from "@/composables/useTemplateCatalog";
import type { TemplateMetadata } from "@/lib/template-catalog";
import { Button } from "@/components/ui/button";

const query = shallowRef("");
const activeTag = shallowRef("all");
const currentPage = shallowRef(1);
const pageSize = 24;
const selectedTemplate = shallowRef<TemplateMetadata | null>(null);
const isDetailOpen = shallowRef(false);

const { templates, pagination, isLoading, error, loadTemplates } = useTemplateCatalog();

const tags = computed(() => {
  const uniqueTags = new Set(templates.value.flatMap((template) => template.tags));
  return [...uniqueTags].sort((left, right) => left.localeCompare(right));
});

const totalTemplates = computed(() => pagination.value.total);

let filterTimer: ReturnType<typeof setTimeout> | undefined;

function loadCurrentPage() {
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
  loadCurrentPage();
}

function selectTemplate(template: TemplateMetadata) {
  selectedTemplate.value = template;
  isDetailOpen.value = true;
}

function clearFilters() {
  query.value = "";
  activeTag.value = "all";
}

watch(
  [query, activeTag],
  () => {
    currentPage.value = 1;
    if (filterTimer) clearTimeout(filterTimer);
    filterTimer = setTimeout(loadCurrentPage, 300);
  },
  { immediate: true },
);

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
  <main class="min-h-[calc(100svh-4rem)] border-b border-border">
    <div class="mx-auto w-full max-w-[1200px] px-5 py-12 sm:px-8 lg:py-16">
      <header
        class="grid gap-8 border-b border-border pb-12 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end"
      >
        <div class="max-w-2xl">
          <p class="font-mono text-[11px] uppercase tracking-[0.08em] text-signal">
            Template registry
          </p>
          <h1 class="mt-4 text-3xl font-medium text-foreground sm:text-4xl">
            Deploy from a proven starting point.
          </h1>
          <p class="mt-4 max-w-xl text-base leading-7 text-muted-foreground">
            Browse deploy-ready service blueprints maintained for Ignitify. Pick a template to
            inspect its configuration, source links, and launch files.
          </p>
        </div>

        <dl
          class="grid grid-cols-2 gap-x-8 gap-y-5 border-l border-border pl-5 sm:gap-x-10 sm:pl-6"
        >
          <div>
            <dt class="font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
              Available
            </dt>
            <dd class="mt-1 text-2xl font-medium tabular-nums text-foreground">
              {{ isLoading ? "..." : totalTemplates }}
            </dd>
          </div>
          <div>
            <dt class="font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
              Source
            </dt>
            <dd class="mt-1 flex items-center gap-1.5 text-sm font-medium text-foreground">
              <PackageOpen class="size-3.5 text-signal" aria-hidden="true" />
              Catalog API
            </dd>
          </div>
        </dl>
      </header>

      <section class="pt-8" aria-labelledby="template-catalog-heading">
        <h2 id="template-catalog-heading" class="sr-only">Available deploy templates</h2>
        <TemplateFilterBar
          :query="query"
          :active-tag="activeTag"
          :tags="tags"
          :result-count="templates.length"
          :total-count="totalTemplates"
          @update-query="query = $event"
          @update-tag="activeTag = $event"
          @clear="clearFilters"
        />

        <div class="mt-8" aria-live="polite">
          <TemplateCatalogGrid
            :templates="templates"
            :is-loading="isLoading"
            :error="error"
            @select="selectTemplate"
            @retry="loadCurrentPage"
          />
        </div>

        <nav
          v-if="pagination.totalPages > 1"
          class="mt-8 flex items-center justify-between gap-4 border border-border bg-card px-4 py-3 max-[640px]:items-start max-[640px]:flex-col"
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
      </section>
    </div>

    <TemplateDetailDialog v-model:open="isDetailOpen" :template="selectedTemplate" />
  </main>
</template>
