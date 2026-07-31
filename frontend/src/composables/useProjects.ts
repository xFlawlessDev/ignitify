import { shallowRef } from "vue";
import { apiCreateProject, apiListProjects } from "@/lib/api/projects";
import type { ProjectInput, ProjectSummary } from "@/lib/types";

export function useProjects() {
  const data = shallowRef<ProjectSummary[]>([]);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);

  async function load() {
    loading.value = true;
    error.value = null;
    const result = await apiListProjects();
    loading.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not load projects";
      return;
    }
    data.value = result.data;
  }

  async function create(input: ProjectInput): Promise<ProjectSummary | null> {
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
