<script setup lang="ts">
import { GitBranch, KeyRound, Server, ShieldCheck } from "@lucide/vue";
import type { Component } from "vue";
import type { ProviderKind } from "@/lib/types";

const emit = defineEmits<{ select: [kind: ProviderKind] }>();

const providerTypes: Array<{
  kind: ProviderKind;
  label: string;
  description: string;
  icon: Component;
}> = [
  {
    kind: "github",
    label: "GitHub",
    description: "OAuth app or GitHub App",
    icon: GitBranch,
  },
  {
    kind: "gitlab",
    label: "GitLab",
    description: "OAuth app for cloud or self-hosted",
    icon: ShieldCheck,
  },
  {
    kind: "gitea",
    label: "Gitea",
    description: "OAuth2 app for a Gitea instance",
    icon: Server,
  },
  {
    kind: "git",
    label: "Generic Git",
    description: "Token-based repository access",
    icon: KeyRound,
  },
];
</script>

<template>
  <section class="grid gap-3" aria-labelledby="provider-types-title">
    <div>
      <p class="ui-label">Add connection</p>
      <h2 id="provider-types-title" class="mt-2 text-base font-medium">Choose a provider</h2>
    </div>
    <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
      <button
        v-for="provider in providerTypes"
        :key="provider.kind"
        class="group grid min-h-[112px] gap-4 rounded-[10px] border border-border bg-card p-4 text-left transition-colors hover:border-foreground/40 hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
        type="button"
        @click="emit('select', provider.kind)"
      >
        <span class="flex items-center justify-between">
          <span class="grid size-8 place-items-center rounded-[6px] border border-border bg-muted">
            <component
              :is="provider.icon"
              class="size-4 text-muted-foreground"
              :stroke-width="1.5"
            />
          </span>
          <GitBranch
            class="size-4 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
            :stroke-width="1.5"
          />
        </span>
        <span class="grid gap-1">
          <strong class="text-[13px] font-medium">{{ provider.label }}</strong>
          <span class="text-[11px] leading-4 text-muted-foreground">{{
            provider.description
          }}</span>
        </span>
      </button>
    </div>
  </section>
</template>
