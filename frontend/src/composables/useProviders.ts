import { shallowRef } from "vue";
import {
  apiCreateProvider,
  apiDeleteProvider,
  apiListProviders,
  apiStartGithubAppManifest,
  apiUpdateProvider,
} from "@/lib/api/providers";
import type {
  GithubManifestInput,
  GithubManifestStart,
  ProviderInput,
  ProviderSummary,
} from "@/lib/types";

export function useProviders() {
  const data = shallowRef<ProviderSummary[]>([]);
  const loading = shallowRef(false);
  const saving = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let loadGeneration = 0;

  async function load() {
    const generation = ++loadGeneration;
    loading.value = true;
    error.value = null;
    const result = await apiListProviders();
    if (generation !== loadGeneration) return;
    loading.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not load providers";
      return;
    }
    data.value = result.data;
  }

  async function create(input: ProviderInput): Promise<ProviderSummary | null> {
    saving.value = true;
    error.value = null;
    const result = await apiCreateProvider(input);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not connect provider";
      return null;
    }
    data.value = [result.data, ...data.value];
    return result.data;
  }

  async function startGithubAppManifest(
    input: GithubManifestInput,
  ): Promise<GithubManifestStart | null> {
    saving.value = true;
    error.value = null;
    const result = await apiStartGithubAppManifest(input);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not start GitHub App connection";
      return null;
    }
    return result.data;
  }

  async function update(providerId: string, input: ProviderInput): Promise<ProviderSummary | null> {
    saving.value = true;
    error.value = null;
    const result = await apiUpdateProvider(providerId, input);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not update provider";
      return null;
    }
    data.value = data.value.map((provider) =>
      provider.id === providerId ? result.data : provider,
    );
    return result.data;
  }

  async function remove(providerId: string): Promise<boolean> {
    saving.value = true;
    error.value = null;
    const result = await apiDeleteProvider(providerId);
    saving.value = false;
    if (!result.success) {
      error.value = result.error ?? "Could not remove provider";
      return false;
    }
    data.value = data.value.filter((provider) => provider.id !== providerId);
    return true;
  }

  return { data, loading, saving, error, load, create, startGithubAppManifest, update, remove };
}
