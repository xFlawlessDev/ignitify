<script setup lang="ts">
import { GitBranch, KeyRound, Link2, LockKeyhole, ShieldCheck } from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type {
  GithubManifestInput,
  ProviderAuthMode,
  ProviderInput,
  ProviderKind,
} from "@/lib/types";

const props = withDefaults(
  defineProps<{
    kind: ProviderKind;
    error?: string | null;
    saving?: boolean;
  }>(),
  { error: null, saving: false },
);

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{
  connect: [input: ProviderInput];
  "connect-github-app": [input: GithubManifestInput];
}>();

const name = shallowRef("");
const authMode = shallowRef<ProviderAuthMode>("oauth");
const baseUrl = shallowRef("");
const internalUrl = shallowRef("");
const redirectUri = shallowRef("");
const clientId = shallowRef("");
const clientSecret = shallowRef("");
const groupNames = shallowRef("");
const username = shallowRef("");
const token = shallowRef("");

const providerLabels: Record<ProviderKind, string> = {
  git: "Generic Git",
  gitea: "Gitea",
  gitlab: "GitLab",
  github: "GitHub",
};

const providerLabel = computed(() => providerLabels[props.kind]);
const isOAuth = computed(() => authMode.value === "oauth");
const isGithubApp = computed(() => authMode.value === "github_app");
const hasInternalUrl = computed(() => props.kind === "gitlab" || props.kind === "gitea");
const urlLabel = computed(() => `${providerLabel.value} URL`);
const callbackPlaceholder = computed(
  () => `https://webserver.domain/api/providers/${props.kind}/callback`,
);

function resetForKind(kind: ProviderKind) {
  authMode.value = kind === "git" ? "token" : "oauth";
  baseUrl.value =
    kind === "github"
      ? "https://github.com"
      : kind === "gitlab"
        ? "https://gitlab.com"
        : kind === "gitea"
          ? "https://gitea.com"
          : "";
  internalUrl.value = "";
  redirectUri.value = "";
  clientId.value = "";
  clientSecret.value = "";
  groupNames.value = "";
  username.value = "";
  token.value = "";
}

watch(
  () => props.kind,
  (kind) => resetForKind(kind),
  { immediate: true },
);

function selectAuthMode(mode: ProviderAuthMode) {
  authMode.value = mode;
}

function submit() {
  if (isGithubApp.value) {
    const directInput = {
      name: name.value.trim(),
      base_url: baseUrl.value.trim(),
    };
    if (!directInput.name || !directInput.base_url) return;
    emit("connect-github-app", directInput);
    return;
  }
  const input: ProviderInput = {
    name: name.value.trim(),
    kind: props.kind,
    auth_mode: authMode.value,
    base_url: baseUrl.value.trim(),
    internal_url: internalUrl.value.trim() || undefined,
    redirect_uri: redirectUri.value.trim() || undefined,
    client_id: clientId.value.trim() || undefined,
    client_secret: clientSecret.value,
    group_names: groupNames.value.trim() || undefined,
    username: username.value.trim() || undefined,
    token: token.value,
  };
  if (!input.name || !input.base_url) return;
  emit("connect", input);
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      class="max-h-[calc(100vh-2rem)] overflow-y-auto rounded-[10px] shadow-none sm:max-w-xl"
    >
      <DialogHeader>
        <div class="mb-1 flex size-9 items-center justify-center border border-border bg-muted">
          <GitBranch class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </div>
        <DialogTitle>{{ providerLabel }} Provider</DialogTitle>
        <DialogDescription>
          Configure the credentials Ignitify will use to browse repositories for app services.
        </DialogDescription>
      </DialogHeader>

      <div class="border-y border-border py-3 text-xs leading-5 text-muted-foreground">
        <p class="font-medium text-foreground">Before you start</p>
        <ol class="mt-1.5 grid list-inside list-decimal gap-0.5">
          <template v-if="props.kind === 'gitlab'">
            <li>Open your GitLab profile settings and navigate to Applications.</li>
            <li>
              Create an application named Ignitify with
              <code class="font-mono text-[10px]">api, read_user, read_repository</code>.
            </li>
            <li>Copy the Application ID and Secret into this form.</li>
          </template>
          <template v-else-if="props.kind === 'gitea'">
            <li>Open Gitea settings and navigate to Applications.</li>
            <li>Create a new OAuth2 application with the callback URI below.</li>
            <li>Copy the Client ID and Secret into this form.</li>
          </template>
          <template v-else-if="props.kind === 'github'">
            <li>Choose OAuth App to use an existing GitHub OAuth application.</li>
            <li>Choose GitHub App to let Ignitify create and connect the App automatically.</li>
          </template>
          <template v-else>
            <li>
              Use a personal access token that can read the repositories Ignitify will deploy.
            </li>
          </template>
        </ol>
      </div>

      <form class="grid gap-4" @submit.prevent="submit">
        <div class="grid gap-2">
          <Label for="provider-name">Name</Label>
          <Input
            id="provider-name"
            v-model="name"
            :maxlength="isGithubApp ? 34 : 100"
            autocomplete="off"
            placeholder="my-personal-account"
          />
        </div>

        <div class="grid gap-2">
          <Label for="provider-url">{{ urlLabel }}</Label>
          <div class="relative">
            <Link2
              class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
              :stroke-width="1.5"
            />
            <Input
              id="provider-url"
              v-model="baseUrl"
              class="pl-9"
              type="url"
              autocomplete="url"
              :disabled="isGithubApp"
              :placeholder="props.kind === 'git' ? 'https://git.example.com' : baseUrl"
            />
          </div>
        </div>

        <div v-if="hasInternalUrl" class="grid gap-2">
          <Label for="provider-internal-url">
            Internal URL <span class="font-normal text-muted-foreground">(optional)</span>
          </Label>
          <Input
            id="provider-internal-url"
            v-model="internalUrl"
            type="url"
            placeholder="http://gitlab:80"
          />
          <p class="text-[11px] leading-4 text-muted-foreground">
            Used for OAuth token exchange when the provider runs on the same private network.
          </p>
        </div>

        <div v-if="props.kind === 'github'" class="grid gap-2">
          <Label>Authentication method</Label>
          <div
            class="grid grid-cols-2 border border-border p-0.5"
            role="group"
            aria-label="Authentication method"
          >
            <button
              class="min-h-8 px-3 text-xs transition-colors"
              :class="
                isOAuth ? 'bg-foreground text-background' : 'text-muted-foreground hover:bg-muted'
              "
              type="button"
              :aria-pressed="isOAuth"
              @click="selectAuthMode('oauth')"
            >
              OAuth App
            </button>
            <button
              class="min-h-8 px-3 text-xs transition-colors"
              :class="
                isGithubApp
                  ? 'bg-foreground text-background'
                  : 'text-muted-foreground hover:bg-muted'
              "
              type="button"
              :aria-pressed="isGithubApp"
              @click="selectAuthMode('github_app')"
            >
              GitHub App
            </button>
          </div>
        </div>

        <template v-if="isOAuth">
          <div class="grid gap-2">
            <Label for="provider-redirect">Redirect URI</Label>
            <Input
              id="provider-redirect"
              v-model="redirectUri"
              :placeholder="callbackPlaceholder"
            />
            <p class="text-[11px] leading-4 text-muted-foreground">
              Register this exact URI in the provider application settings.
            </p>
          </div>
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="provider-client-id">{{
                props.kind === "gitlab" ? "Application ID" : "Client ID"
              }}</Label>
              <Input id="provider-client-id" v-model="clientId" autocomplete="off" />
            </div>
            <div class="grid gap-2">
              <Label for="provider-client-secret">{{
                props.kind === "gitlab" ? "Application Secret" : "Client Secret"
              }}</Label>
              <Input
                id="provider-client-secret"
                v-model="clientSecret"
                type="password"
                autocomplete="new-password"
              />
            </div>
          </div>
          <div v-if="props.kind === 'gitlab'" class="grid gap-2">
            <Label for="provider-groups">
              Group Name
              <span class="font-normal text-muted-foreground">(optional, comma-separated)</span>
            </Label>
            <Input id="provider-groups" v-model="groupNames" placeholder="platform, web" />
          </div>
        </template>

        <template v-else-if="isGithubApp">
          <div class="border border-border bg-muted/30 px-4 py-4">
            <div class="flex items-start gap-3">
              <ShieldCheck
                class="mt-0.5 size-4 shrink-0 text-muted-foreground"
                :stroke-width="1.5"
              />
              <div class="grid gap-1.5">
                <p class="text-sm font-medium">Create and connect a GitHub App</p>
                <p class="text-xs leading-5 text-muted-foreground">
                  GitHub will generate the App ID, client secret, and private key for Ignitify. You
                  will review the requested repository permissions on GitHub before approving.
                </p>
              </div>
            </div>
          </div>
        </template>

        <template v-else>
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="provider-username">
                Username <span class="font-normal text-muted-foreground">(optional)</span>
              </Label>
              <Input id="provider-username" v-model="username" autocomplete="username" />
            </div>
            <div class="grid gap-2">
              <Label for="provider-token">Access token</Label>
              <div class="relative">
                <KeyRound
                  class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
                  :stroke-width="1.5"
                />
                <Input
                  id="provider-token"
                  v-model="token"
                  class="pl-9"
                  type="password"
                  autocomplete="new-password"
                />
              </div>
            </div>
          </div>
        </template>

        <p class="flex items-start gap-2 text-[11px] leading-4 text-muted-foreground">
          <LockKeyhole class="mt-0.5 size-3.5 shrink-0" :stroke-width="1.5" />
          Secrets are encrypted at rest and never returned to the browser after saving.
        </p>
        <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>

        <DialogFooter class="mt-1">
          <Button type="submit" :disabled="props.saving">
            {{
              props.saving
                ? "Connecting..."
                : isGithubApp
                  ? "Connect directly to GitHub"
                  : `Connect ${providerLabel}`
            }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
