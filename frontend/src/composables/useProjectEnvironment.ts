import { shallowRef } from "vue";
import { apiGetProjectEnvironment, apiUpdateProjectEnvironment } from "@/lib/api/projects";
import type { ProjectEnvironmentResponse, ProjectEnvironmentVariableInput } from "@/lib/types";

const emptyEnvironment: ProjectEnvironmentResponse = {
  role: "viewer",
  variables: [],
};

export function useProjectEnvironment() {
  const data = shallowRef<ProjectEnvironmentResponse>(emptyEnvironment);
  const loading = shallowRef(false);
  const saving = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load(projectId: string) {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const result = await apiGetProjectEnvironment(projectId);
    if (generation !== loadGeneration) return;
    loading.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not load project environment";
      data.value = emptyEnvironment;
      return;
    }
    data.value = result.data;
  }

  async function save(
    projectId: string,
    variables: ProjectEnvironmentVariableInput[],
  ): Promise<boolean> {
    saving.value = true;
    error.value = null;
    const result = await apiUpdateProjectEnvironment(projectId, variables);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not save project environment";
      return false;
    }
    data.value = result.data;
    return true;
  }

  return { data, loading, saving, error, load, save };
}
