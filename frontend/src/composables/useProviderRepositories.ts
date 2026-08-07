import { shallowRef } from "vue";
import { apiListProviderBranches, apiListProviderRepositories } from "@/lib/api/providers";
import type { ProviderBranch, ProviderRepository } from "@/lib/types";

export function useProviderRepositories() {
  const repositories = shallowRef<ProviderRepository[]>([]);
  const branches = shallowRef<ProviderBranch[]>([]);
  const repositoriesLoading = shallowRef(false);
  const branchesLoading = shallowRef(false);
  const repositoriesError = shallowRef<string | null>(null);
  const branchesError = shallowRef<string | null>(null);
  let repositoryGeneration = 0;
  let branchGeneration = 0;

  async function loadRepositories(providerId: string) {
    const generation = ++repositoryGeneration;
    branchGeneration += 1;
    repositories.value = [];
    branches.value = [];
    repositoriesError.value = null;
    branchesError.value = null;
    if (!providerId) return;
    repositoriesLoading.value = true;
    try {
      const result = await apiListProviderRepositories(providerId);
      if (generation !== repositoryGeneration) return;
      if (!result.success) {
        repositoriesError.value = result.error ?? "Could not load repositories";
        return;
      }
      repositories.value = result.data;
    } catch (cause) {
      if (generation !== repositoryGeneration) return;
      repositoriesError.value =
        cause instanceof Error ? cause.message : "Could not load repositories";
    } finally {
      if (generation === repositoryGeneration) repositoriesLoading.value = false;
    }
  }

  async function loadBranches(providerId: string, repository: string) {
    const generation = ++branchGeneration;
    branches.value = [];
    branchesError.value = null;
    if (!providerId || !repository) return;
    branchesLoading.value = true;
    try {
      const result = await apiListProviderBranches(providerId, repository);
      if (generation !== branchGeneration) return;
      if (!result.success) {
        branchesError.value = result.error ?? "Could not load branches";
        return;
      }
      branches.value = result.data;
    } catch (cause) {
      if (generation !== branchGeneration) return;
      branchesError.value = cause instanceof Error ? cause.message : "Could not load branches";
    } finally {
      if (generation === branchGeneration) branchesLoading.value = false;
    }
  }

  function reset() {
    repositoryGeneration += 1;
    branchGeneration += 1;
    repositories.value = [];
    branches.value = [];
    repositoriesLoading.value = false;
    branchesLoading.value = false;
    repositoriesError.value = null;
    branchesError.value = null;
  }

  return {
    repositories,
    branches,
    repositoriesLoading,
    branchesLoading,
    repositoriesError,
    branchesError,
    loadRepositories,
    loadBranches,
    reset,
  };
}
