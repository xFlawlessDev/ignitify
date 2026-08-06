<script setup lang="ts">
import { CircleCheck, GitBranch, KeyRound, Plus, RefreshCw, ShieldCheck } from "@lucide/vue";
import { computed, onMounted, shallowRef } from "vue";
import ProviderConnectDialog from "@/components/provider/ProviderConnectDialog.vue";
import ProviderList from "@/components/provider/ProviderList.vue";
import ProviderTypeGrid from "@/components/provider/ProviderTypeGrid.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useProviders } from "@/composables/useProviders";
import type {
  GithubManifestInput,
  ProviderInput,
  ProviderKind,
  ProviderSummary,
} from "@/lib/types";
import { useRoute } from "vue-router";
import { useAuthStore } from "@/stores/auth";

const { data, error, load, loading, saving, create, startGithubAppManifest, remove } =
  useProviders();
const auth = useAuthStore();
const route = useRoute();
const connectOpen = shallowRef(false);
const selectedKind = shallowRef<ProviderKind>("github");
const providerCount = computed(() => data.value.length);
const configuredCount = computed(
  () => data.value.filter((provider) => provider.token_configured).length,
);
const providerKinds = computed(() => new Set(data.value.map((provider) => provider.kind)).size);
const githubConnectionStatus = computed(() => {
  if (route.query.github === "connected") return "GitHub App connected";
  if (route.query.github === "cancelled") return "GitHub App connection cancelled";
  return null;
});

async function connectProvider(input: ProviderInput) {
  const provider = await create(input);
  if (!provider) return;
  connectOpen.value = false;
}

async function connectGithubApp(input: GithubManifestInput) {
  const manifestStart = await startGithubAppManifest(input);
  if (!manifestStart) return;

  const form = document.createElement("form");
  form.method = "POST";
  form.action = manifestStart.action_url;
  form.hidden = true;
  const manifest = document.createElement("input");
  manifest.type = "hidden";
  manifest.name = "manifest";
  manifest.value = JSON.stringify(manifestStart.manifest);
  form.append(manifest);
  document.body.append(form);
  form.submit();
}

function openProvider(kind: ProviderKind) {
  selectedKind.value = kind;
  connectOpen.value = true;
}

async function removeProvider(provider: ProviderSummary) {
  if (
    !window.confirm(`Remove ${provider.name}? Existing services will no longer be able to use it.`)
  ) {
    return;
  }
  await remove(provider.id);
}

onMounted(load);
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-5 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
    >
      <div>
        <p class="ui-label">Workspace</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Providers</h1>
        <p class="mt-2.5 max-w-[56ch] text-[13px] leading-5 text-muted-foreground">
          Connect source-control accounts once, then select their repositories when configuring an
          app.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <Button
          v-if="auth.isAdmin"
          class="order-1 w-full sm:order-none sm:w-auto"
          @click="openProvider('github')"
        >
          <Plus class="size-4" :stroke-width="1.5" />
          Connect GitHub
        </Button>
        <button
          class="grid size-9 shrink-0 place-items-center rounded-[3px] border border-border bg-card text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          type="button"
          aria-label="Refresh providers"
          title="Refresh providers"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        </button>
      </div>
    </header>

    <section
      class="mt-[22px] grid overflow-hidden divide-y divide-border border border-border bg-card sm:grid-cols-3 sm:divide-x sm:divide-y-0"
      aria-label="Provider summary"
    >
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <GitBranch class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Connections</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ providerCount }}</strong>
        </div>
      </div>
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <KeyRound class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Credentials ready</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ configuredCount }}</strong>
        </div>
      </div>
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <ShieldCheck class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Provider types</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ providerKinds }}</strong>
        </div>
      </div>
    </section>

    <p v-if="error && !loading" class="mt-4 text-xs text-destructive" role="alert">{{ error }}</p>
    <p
      v-if="githubConnectionStatus"
      class="mt-4 flex items-center gap-2 border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-700 dark:text-emerald-300"
      role="status"
    >
      <CircleCheck class="size-4 shrink-0" :stroke-width="1.5" />
      {{ githubConnectionStatus }}
    </p>

    <ProviderTypeGrid v-if="auth.isAdmin" class="mt-[22px]" @select="openProvider" />

    <section
      v-if="loading"
      class="mt-[22px] border border-border bg-card"
      role="status"
      aria-label="Loading providers"
    >
      <div
        v-for="index in 3"
        :key="index"
        class="flex min-h-[90px] items-center gap-3.5 border-b border-border px-4 py-4 last:border-b-0 sm:px-5"
      >
        <Skeleton class="size-9 shrink-0 rounded-[4px]" />
        <div class="grid min-w-0 flex-1 gap-2">
          <Skeleton class="h-3 w-40 max-w-full" />
          <Skeleton class="h-2.5 w-64 max-w-full" />
          <Skeleton class="h-2.5 w-44 max-w-full" />
        </div>
      </div>
    </section>

    <section
      v-else-if="data.length === 0"
      class="mt-[22px] border border-border bg-card px-5 py-10"
    >
      <div class="max-w-lg">
        <CircleCheck class="size-5 text-muted-foreground" :stroke-width="1.5" />
        <h2 class="mt-3 text-base font-medium">No providers connected</h2>
        <p class="mt-1 text-xs leading-5 text-muted-foreground">
          Add a Git, Gitea, or GitLab connection to make private repositories available to app
          services.
        </p>
        <Button v-if="auth.isAdmin" class="mt-5" size="sm" @click="openProvider('github')">
          <Plus class="size-4" :stroke-width="1.5" />
          Add first provider
        </Button>
        <p v-else class="mt-5 text-xs text-muted-foreground">
          Ask an administrator to connect a provider.
        </p>
      </div>
    </section>

    <div v-else class="mt-[22px] grid gap-6 lg:grid-cols-[minmax(0,1fr)_280px] lg:items-start">
      <section class="grid gap-3" aria-labelledby="connected-providers-title">
        <div class="flex items-end justify-between gap-4">
          <div>
            <p class="ui-label">Source control</p>
            <h2 id="connected-providers-title" class="mt-2 text-base font-medium">
              Connected providers
            </h2>
          </div>
          <span class="font-mono text-[11px] text-muted-foreground">{{ providerCount }} total</span>
        </div>
        <ProviderList
          :providers="data"
          :busy="saving"
          :can-manage="auth.isAdmin"
          @remove="removeProvider"
        />
      </section>

      <aside class="border border-border bg-card px-5 py-5" aria-labelledby="provider-access-title">
        <p class="ui-label">Repository access</p>
        <h2 id="provider-access-title" class="mt-2 text-base font-medium">Ready for app setup</h2>
        <p class="mt-2 text-xs leading-5 text-muted-foreground">
          Providers are shared connection points. App services can use them to browse repositories
          and branches without exposing tokens to the UI.
        </p>
        <div class="mt-5 grid gap-3 border-t border-border pt-4">
          <div class="flex items-start gap-2.5">
            <ShieldCheck class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
            <p class="text-[11px] leading-4 text-muted-foreground">
              Credentials are encrypted at rest.
            </p>
          </div>
          <div class="flex items-start gap-2.5">
            <GitBranch class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
            <p class="text-[11px] leading-4 text-muted-foreground">
              GitLab, Gitea, and generic Git endpoints are supported.
            </p>
          </div>
        </div>
      </aside>
    </div>

    <ProviderConnectDialog
      v-model:open="connectOpen"
      :kind="selectedKind"
      :error="error"
      :saving="saving"
      @connect="connectProvider"
      @connect-github-app="connectGithubApp"
    />
  </div>
</template>
