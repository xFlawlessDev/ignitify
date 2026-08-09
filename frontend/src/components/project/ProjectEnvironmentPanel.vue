<script setup lang="ts">
import { Copy, Eye, EyeOff, KeyRound, LockKeyhole, Plus, Trash2 } from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { ProjectEnvironmentVariable, ProjectEnvironmentVariableInput } from "@/lib/types";

const props = withDefaults(
  defineProps<{
    variables: ProjectEnvironmentVariable[];
    canManage?: boolean;
    loading?: boolean;
    saving?: boolean;
    error?: string | null;
  }>(),
  { canManage: false, saving: false, error: null },
);

const emit = defineEmits<{
  save: [variables: ProjectEnvironmentVariableInput[]];
}>();

interface EnvironmentDraft extends ProjectEnvironmentVariable {
  value: string;
}

const draft = shallowRef<EnvironmentDraft[]>([]);
const activeKind = shallowRef<"variables" | "secrets">("variables");
const showSecrets = shallowRef(false);
const validationError = shallowRef<string | null>(null);
const copiedKey = shallowRef<string | null>(null);
const variableCount = computed(() => draft.value.filter((variable) => !variable.is_secret).length);
const secretCount = computed(() => draft.value.filter((variable) => variable.is_secret).length);
const activeRows = computed(() =>
  draft.value
    .map((variable, index) => ({ variable, index }))
    .filter(({ variable }) =>
      activeKind.value === "secrets" ? variable.is_secret : !variable.is_secret,
    ),
);

watch(
  () => props.variables,
  (variables) => {
    draft.value = variables.map((variable) => ({ ...variable, value: variable.value ?? "" }));
    if (
      !variables.some((variable) =>
        activeKind.value === "secrets" ? variable.is_secret : !variable.is_secret,
      )
    ) {
      activeKind.value = variables.some((variable) => variable.is_secret) ? "secrets" : "variables";
    }
    validationError.value = null;
  },
  { immediate: true, deep: true },
);

function addVariable(isSecret: boolean) {
  activeKind.value = isSecret ? "secrets" : "variables";
  draft.value = [...draft.value, { key: "", value: "", is_secret: isSecret, is_set: false }];
}

function removeVariable(index: number) {
  draft.value = draft.value.filter((_, itemIndex) => itemIndex !== index);
}

function updateSecret(index: number, isSecret: boolean) {
  const variable = draft.value[index];
  if (!variable) return;
  variable.is_secret = isSecret;
  activeKind.value = isSecret ? "secrets" : "variables";
  draft.value = [...draft.value];
}

async function copyValue(variable: ProjectEnvironmentVariable) {
  if (variable.is_secret && !showSecrets.value) return;
  if (!variable.value) return;
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(variable.value);
    } else {
      const input = document.createElement("textarea");
      input.value = variable.value;
      input.setAttribute("readonly", "true");
      input.style.position = "fixed";
      input.style.opacity = "0";
      document.body.append(input);
      input.select();
      document.execCommand("copy");
      input.remove();
    }
  } catch {
    return;
  }
  copiedKey.value = variable.key;
  window.setTimeout(() => {
    if (copiedKey.value === variable.key) copiedKey.value = null;
  }, 1600);
}

function submit() {
  const normalized = draft.value.map((variable) => ({
    ...variable,
    key: variable.key.trim().toUpperCase(),
  }));
  if (normalized.some((variable) => !/^[A-Z_][A-Z0-9_]*$/.test(variable.key))) {
    validationError.value = "Use uppercase letters, numbers, and underscores for variable keys.";
    return;
  }
  const keys = new Set<string>();
  if (normalized.some((variable) => keys.has(variable.key) || !keys.add(variable.key))) {
    validationError.value = "Each project variable key must be unique.";
    return;
  }
  if (
    normalized.some(
      (variable) => !variable.value.trim() && !(variable.is_secret && variable.is_set),
    )
  ) {
    validationError.value = "Add a value for every new project variable before saving.";
    return;
  }
  validationError.value = null;
  emit(
    "save",
    normalized.map(({ key, value, is_secret, is_set }) => ({
      key,
      is_secret,
      value: is_secret && is_set && !value ? undefined : value,
    })),
  );
}
</script>

<template>
  <section class="app-surface" aria-labelledby="project-environment-title">
    <div
      class="app-panel-header flex items-start justify-between gap-4 px-5 pt-5 pb-4 max-[560px]:flex-col"
    >
      <div class="flex min-w-0 items-start gap-3">
        <span
          class="grid size-8 shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
        >
          <KeyRound class="size-4" :stroke-width="1.5" />
        </span>
        <div class="min-w-0">
          <p class="ui-label">Shared configuration</p>
          <h2 id="project-environment-title" class="mt-2 text-xl leading-none font-normal">
            Environment
          </h2>
          <p class="mt-2 max-w-[58ch] text-xs leading-5 text-muted-foreground">
            Shared variables apply to every service. Secrets stay masked and can be overridden at
            service level.
          </p>
        </div>
      </div>
      <div class="grid shrink-0 gap-2 max-[560px]:w-full">
        <Tabs
          :model-value="activeKind"
          class="max-[560px]:w-full"
          @update:model-value="(value) => (activeKind = value as 'variables' | 'secrets')"
        >
          <TabsList class="h-8 w-full rounded-[4px] sm:w-auto">
            <TabsTrigger value="variables" class="min-w-28 px-3 text-[11px]">
              Variables
              <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                variableCount
              }}</span>
            </TabsTrigger>
            <TabsTrigger value="secrets" class="min-w-28 px-3 text-[11px]">
              <LockKeyhole class="size-3.5" :stroke-width="1.5" />
              Secrets
              <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                secretCount
              }}</span>
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <Button
          variant="ghost"
          v-if="props.canManage && activeKind === 'secrets' && secretCount"
          class="inline-flex items-center justify-end gap-1.5 py-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
          type="button"
          @click="showSecrets = !showSecrets"
        >
          <EyeOff v-if="showSecrets" class="size-3.5" :stroke-width="1.5" />
          <Eye v-else class="size-3.5" :stroke-width="1.5" />
          {{ showSecrets ? "Hide values" : "Reveal values" }}
        </Button>
      </div>
    </div>

    <div
      v-if="props.loading"
      class="divide-y divide-border px-5"
      role="status"
      aria-label="Loading environment"
    >
      <div v-for="index in 3" :key="index" class="grid gap-2 py-4">
        <Skeleton class="h-3 w-36 max-w-full" />
        <Skeleton class="h-2.5 w-56 max-w-full" />
      </div>
    </div>
    <div v-else-if="props.canManage && activeRows.length" class="px-5 py-2">
      <div
        class="hidden grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] gap-2 py-2 text-[10px] uppercase text-muted-foreground sm:grid"
      >
        <span>Key</span>
        <span>Value</span>
        <span>Type</span>
        <span class="sr-only">Actions</span>
      </div>
      <div
        v-for="{ variable, index } in activeRows"
        :key="variable.key + '-' + index"
        class="grid min-h-[58px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] items-end gap-2 border-b border-border py-2.5 last:border-b-0 max-[560px]:grid-cols-[minmax(0,1fr)_auto_auto]"
      >
        <Label class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground">
          Key
          <Input
            v-model="variable.key"
            class="h-8 font-mono text-xs uppercase"
            autocomplete="off"
          />
        </Label>
        <Label
          class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground max-[560px]:col-span-3"
        >
          Value
          <Input
            v-model="variable.value"
            class="h-8 font-mono text-xs"
            :type="variable.is_secret && !showSecrets ? 'password' : 'text'"
            :placeholder="
              variable.is_secret && variable.is_set
                ? 'Stored securely; enter replacement'
                : 'Enter value'
            "
            autocomplete="off"
          />
        </Label>
        <div class="grid gap-1.5 text-[11px] text-muted-foreground">
          Secret
          <Switch
            :model-value="variable.is_secret"
            :aria-label="'Mark ' + (variable.key || 'variable') + ' secret'"
            @update:model-value="updateSecret(index, $event)"
          />
        </div>
        <Button
          variant="ghost"
          class="grid size-8 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="`Remove ${variable.key || 'variable'}`"
          title="Remove variable"
          @click="removeVariable(index)"
        >
          <Trash2 class="size-3.5" :stroke-width="1.5" />
        </Button>
      </div>
    </div>

    <div v-else-if="props.canManage" class="px-5 py-8">
      <LockKeyhole
        v-if="activeKind === 'secrets'"
        class="mb-3 size-5 text-muted-foreground"
        :stroke-width="1.5"
      />
      <p class="text-sm font-medium">
        {{ activeKind === "secrets" ? "No secrets configured" : "No variables configured" }}
      </p>
      <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
        {{
          activeKind === "secrets"
            ? "Store credentials and tokens here once, then keep them masked across project services."
            : "Add non-sensitive defaults such as APP_ENV or LOG_LEVEL for every service."
        }}
      </p>
    </div>

    <div v-else-if="activeRows.length" class="px-5 py-2">
      <div
        v-for="{ variable } in activeRows"
        :key="variable.key"
        class="grid min-h-[43px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_28px] items-center gap-3 border-b border-border last:border-b-0 max-[480px]:grid-cols-[minmax(0,1fr)_28px]"
      >
        <code
          class="flex min-w-0 items-center gap-1.5 truncate font-mono text-[11px] text-foreground"
        >
          <LockKeyhole
            v-if="variable.is_secret"
            class="size-3 shrink-0 text-muted-foreground"
            :stroke-width="1.5"
          />
          {{ variable.key }}
        </code>
        <span
          class="truncate font-mono text-[11px] text-muted-foreground max-[480px]:col-start-1 max-[480px]:col-end-3 max-[480px]:row-start-2 max-[480px]:pb-2"
          >{{
            variable.is_secret
              ? variable.is_set
                ? "Stored securely"
                : "Not set"
              : variable.value || "Not set"
          }}</span
        >
        <Button
          variant="ghost"
          v-if="!variable.is_secret || showSecrets"
          class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="'Copy ' + variable.key"
          :disabled="!variable.value"
          @click="copyValue(variable)"
        >
          <Copy class="size-3.5" :stroke-width="1.5" />
        </Button>
      </div>
    </div>

    <div v-else class="px-5 py-8">
      <LockKeyhole
        v-if="activeKind === 'secrets'"
        class="mb-3 size-5 text-muted-foreground"
        :stroke-width="1.5"
      />
      <p class="text-sm font-medium">
        {{ activeKind === "secrets" ? "No secrets configured" : "No variables configured" }}
      </p>
      <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
        {{
          activeKind === "secrets"
            ? "Secrets are only available to project managers."
            : "Shared variables will appear here once they are configured."
        }}
      </p>
    </div>

    <div
      v-if="props.canManage"
      class="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-3.5"
    >
      <div class="flex flex-wrap gap-2">
        <Button size="sm" type="button" variant="outline" @click="addVariable(false)">
          <Plus class="size-4" :stroke-width="1.5" />
          Add variable
        </Button>
        <Button size="sm" type="button" variant="outline" @click="addVariable(true)">
          <LockKeyhole class="size-4" :stroke-width="1.5" />
          Add secret
        </Button>
      </div>
      <Button size="sm" type="button" :disabled="props.saving" @click="submit">
        {{ props.saving ? "Saving..." : "Save environment" }}
      </Button>
    </div>

    <p
      v-if="validationError || props.error"
      class="border-t border-destructive/30 bg-destructive/5 px-5 py-2.5 text-xs text-destructive"
      role="alert"
    >
      {{ validationError ?? props.error }}
    </p>
    <p
      v-else-if="copiedKey"
      class="border-t border-border px-5 py-2.5 font-mono text-[11px] text-[var(--status-live)]"
      role="status"
    >
      {{ copiedKey }} copied
    </p>
  </section>
</template>
