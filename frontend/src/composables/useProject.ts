import { shallowRef } from "vue";
import { apiGetProject, apiUpdateProject } from "@/lib/api/projects";
import type { ProjectInput, ProjectSummary } from "@/lib/types";

export function useProject() {
  const data = shallowRef<ProjectSummary | null>(null);
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load(projectId: string) {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const result = await apiGetProject(projectId);
    if (generation !== loadGeneration) return;
    loading.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not load project";
      data.value = null;
      return;
    }
    data.value = result.data;
  }

  async function update(input: ProjectInput): Promise<ProjectSummary | null> {
    if (!data.value) return null;
    error.value = null;
    const result = await apiUpdateProject(data.value.id, input);
    if (!result.success) {
      error.value = result.error ?? "Could not update project";
      return null;
    }
    data.value = result.data;
    return result.data;
  }

  return { data, loading, error, load, update };
}
