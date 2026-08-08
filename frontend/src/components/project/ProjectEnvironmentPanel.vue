<script setup lang="ts">
import { Copy, Eye, EyeOff, KeyRound, Plus, Trash2 } from "@lucide/vue";
import { shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import type { ProjectEnvironmentVariable, ProjectEnvironmentVariableInput } from "@/lib/types";

const props = withDefaults(
  defineProps<{
    variables: ProjectEnvironmentVariable[];
    canManage?: boolean;
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
const showSecrets = shallowRef(false);
const validationError = shallowRef<string | null>(null);
const copiedKey = shallowRef<string | null>(null);

watch(
  () => props.variables,
  (variables) => {
    draft.value = variables.map((variable) => ({ ...variable, value: variable.value ?? "" }));
    validationError.value = null;
  },
  { immediate: true, deep: true },
);

function addVariable() {
  draft.value = [...draft.value, { key: "", value: "", is_secret: true, is_set: false }];
}

function removeVariable(index: number) {
  draft.value = draft.value.filter((_, itemIndex) => itemIndex !== index);
}

function copyValue(variable: ProjectEnvironmentVariable) {
  if (variable.is_secret && !showSecrets.value) return;
  void navigator.clipboard?.writeText(variable.value ?? "");
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
      class="app-panel-header flex items-start justify-between gap-4 px-5 pt-5 pb-4 max-[520px]:flex-col"
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
            Project environment
          </h2>
          <p class="mt-2 max-w-[58ch] text-xs leading-5 text-muted-foreground">
            Defaults applied to every service at deployment. A service can override a key for its
            own deploy.
          </p>
        </div>
      </div>
      <button
        class="inline-flex shrink-0 items-center gap-1.5 py-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
        type="button"
        @click="showSecrets = !showSecrets"
      >
        <EyeOff v-if="showSecrets" class="size-3.5" :stroke-width="1.5" />
        <Eye v-else class="size-3.5" :stroke-width="1.5" />
        {{ showSecrets ? "Hide values" : "Reveal values" }}
      </button>
    </div>

    <div v-if="props.canManage" class="px-5 py-2">
      <div
        v-for="(variable, index) in draft"
        :key="`${variable.key}-${index}`"
        class="grid min-h-[58px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] items-end gap-2 border-b border-border py-2.5 last:border-b-0 max-[560px]:grid-cols-[minmax(0,1fr)_auto_auto]"
      >
        <label class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground">
          Key
          <Input
            v-model="variable.key"
            class="h-8 font-mono text-xs uppercase"
            autocomplete="off"
          />
        </label>
        <label
          class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground max-[560px]:col-span-3"
        >
          Value
          <Input
            v-model="variable.value"
            class="h-8 font-mono text-xs"
            :type="variable.is_secret && !showSecrets ? 'password' : 'text'"
            autocomplete="off"
          />
        </label>
        <label class="grid gap-1.5 text-[11px] text-muted-foreground">
          Secret
          <Switch
            v-model="variable.is_secret"
            :aria-label="`Mark ${variable.key || 'variable'} secret`"
          />
        </label>
        <button
          class="grid size-8 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="`Remove ${variable.key || 'variable'}`"
          title="Remove variable"
          @click="removeVariable(index)"
        >
          <Trash2 class="size-3.5" :stroke-width="1.5" />
        </button>
      </div>
    </div>

    <div v-else-if="variables.length" class="px-5 py-2">
      <div
        v-for="variable in variables"
        :key="variable.key"
        class="grid min-h-[43px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_28px] items-center gap-3 border-b border-border last:border-b-0 max-[480px]:grid-cols-[minmax(0,1fr)_28px]"
      >
        <code class="truncate font-mono text-[11px] text-foreground">{{ variable.key }}</code>
        <span
          class="truncate font-mono text-[11px] text-muted-foreground max-[480px]:col-start-1 max-[480px]:col-end-3 max-[480px]:row-start-2 max-[480px]:pb-2"
          >{{ variable.is_secret && !showSecrets ? "••••••••••••" : variable.value }}</span
        >
        <button
          class="grid size-7 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-border hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="`Copy ${variable.key}`"
          @click="copyValue(variable)"
        >
          <Copy class="size-3.5" :stroke-width="1.5" />
        </button>
      </div>
    </div>

    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No shared variables yet</p>
      <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
        Add values such as DATABASE_URL or APP_ENV once, then reuse them across your services.
      </p>
    </div>

    <div
      v-if="props.canManage"
      class="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-3.5"
    >
      <Button size="sm" type="button" variant="outline" @click="addVariable">
        <Plus class="size-4" :stroke-width="1.5" />
        Add variable
      </Button>
      <Button size="sm" type="button" :disabled="props.saving" @click="submit">
        {{ props.saving ? "Saving..." : "Save project env" }}
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
