import { shallowRef } from "vue";
import { apiCreateProject, apiListProjects } from "@/lib/api/projects";
import type { ProjectInput, ProjectSummary } from "@/lib/types";

export function useProjects() {
  const data = shallowRef<ProjectSummary[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load() {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const result = await apiListProjects();
    if (generation !== loadGeneration) return;
    loading.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not load projects";
      return;
    }
    data.value = result.data;
  }

  async function create(input: ProjectInput): Promise<ProjectSummary | null> {
    loadGeneration += 1;
    error.value = null;
    const result = await apiCreateProject(input);
    if (!result.success) {
      error.value = result.error ?? "Could not create project";
      return null;
    }
    data.value = [result.data, ...data.value];
    return result.data;
  }

  return { data, loading, error, load, create };
}
