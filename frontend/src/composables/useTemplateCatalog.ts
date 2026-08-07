import { shallowRef } from "vue";
import { apiListTemplates, type TemplateCatalogQuery } from "@/lib/api/templates";
import { toTemplateMetadata, type TemplateMetadata } from "@/lib/template-catalog";

export function useTemplateCatalog() {
  const templates = shallowRef<TemplateMetadata[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let generation = 0;

  async function loadTemplates(query: TemplateCatalogQuery = {}) {
    const currentGeneration = ++generation;
    loading.value = true;
    error.value = null;

    try {
      const result = await apiListTemplates(query);
      if (currentGeneration !== generation) return;
      if (!result.success) {
        error.value = result.error ?? "Could not load templates.";
        return;
      }
      const page = Array.isArray(result.data)
        ? {
            items: result.data,
            page: query.page ?? 1,
            pageSize: query.pageSize ?? 24,
            total: result.data.length,
            totalPages: 1,
            hasNextPage: false,
            hasPreviousPage: false,
          }
        : result.data;
      templates.value = page.items.map(toTemplateMetadata);
      pagination.value = page;
    } catch (cause) {
      if (currentGeneration !== generation) return;
      error.value = cause instanceof Error ? cause.message : "Could not load templates.";
    } finally {
      if (currentGeneration === generation) loading.value = false;
    }
  }

  function reset() {
    generation += 1;
    templates.value = [];
    loading.value = false;
    error.value = null;
    pagination.value = {
      page: 1,
      pageSize: 24,
      total: 0,
      totalPages: 0,
      hasNextPage: false,
      hasPreviousPage: false,
    };
  }

  const pagination = shallowRef({
    page: 1,
    pageSize: 24,
    total: 0,
    totalPages: 0,
    hasNextPage: false,
    hasPreviousPage: false,
  });

  return { templates, pagination, loading, isLoading: loading, error, loadTemplates, reset };
}
