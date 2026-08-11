<script setup lang="ts">
import { Bot, KeyRound, RotateCcw, Save, Trash2 } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { apiGetAiSettings, apiUpdateAiSettings, type AiSettings } from "@/lib/api/ai";

interface AiSettingsDraft {
  enabled: boolean;
  baseUrl: string;
  model: string;
  apiKey: string;
  apiKeyConfigured: boolean;
  clearApiKey: boolean;
}

const { t } = useI18n();
const defaults: AiSettingsDraft = {
  enabled: false,
  baseUrl: "",
  model: "",
  apiKey: "",
  apiKeyConfigured: false,
  clearApiKey: false,
};
const draft = reactive<AiSettingsDraft>({ ...defaults });
const saved = shallowRef<AiSettingsDraft | null>(null);
const loadState = shallowRef<"loading" | "idle" | "saving" | "error">("loading");
const requestError = shallowRef("");

function toDraft(settings: AiSettings): AiSettingsDraft {
  return {
    enabled: settings.enabled,
    baseUrl: settings.base_url,
    model: settings.model,
    apiKey: "",
    apiKeyConfigured: settings.api_key_configured,
    clearApiKey: false,
  };
}

function cloneDraft(value: AiSettingsDraft): AiSettingsDraft {
  return { ...value, apiKey: "", clearApiKey: false };
}

const endpointError = computed(() => {
  if (!draft.enabled || draft.baseUrl.trim()) return "";
  return t("ai.settings.errors.baseUrlRequired");
});
const modelError = computed(() => {
  if (!draft.enabled || draft.model.trim()) return "";
  return t("ai.settings.errors.modelRequired");
});
const apiKeyError = computed(() => {
  if (!draft.apiKey) return "";
  return /\s/.test(draft.apiKey) ? t("ai.settings.errors.apiKey") : "";
});
const isDirty = computed(() => {
  if (!saved.value) return false;
  return JSON.stringify({ ...draft, apiKey: "" }) !== JSON.stringify(saved.value);
});
const canSave = computed(
  () =>
    loadState.value !== "loading" &&
    loadState.value !== "saving" &&
    isDirty.value &&
    !endpointError.value &&
    !modelError.value &&
    !apiKeyError.value,
);

function markDirty() {
  if (loadState.value !== "loading" && loadState.value !== "saving") loadState.value = "idle";
  requestError.value = "";
}

function updateEnabled(value: boolean) {
  draft.enabled = value;
  markDirty();
}

function updateApiKey(value: string | number) {
  const apiKey = String(value);
  draft.apiKey = apiKey;
  if (apiKey) {
    draft.clearApiKey = false;
    draft.apiKeyConfigured = true;
  }
  markDirty();
}

function clearApiKey() {
  draft.apiKey = "";
  draft.apiKeyConfigured = false;
  draft.clearApiKey = true;
  markDirty();
}

function reset() {
  if (!saved.value) return;
  Object.assign(draft, cloneDraft(saved.value));
  requestError.value = "";
  loadState.value = "idle";
}

async function load(showSuccess = false) {
  loadState.value = "loading";
  requestError.value = "";
  const result = await apiGetAiSettings();
  if (!result.success) {
    loadState.value = "error";
    requestError.value = result.error ?? t("ai.settings.loadFailed");
    return;
  }
  const next = toDraft(result.data);
  Object.assign(draft, next);
  saved.value = cloneDraft(next);
  loadState.value = "idle";
  if (showSuccess) toast.success(t("ai.settings.refreshed"));
}

async function save() {
  if (!canSave.value) return;
  loadState.value = "saving";
  requestError.value = "";
  const result = await apiUpdateAiSettings({
    enabled: draft.enabled,
    base_url: draft.baseUrl.trim(),
    model: draft.model.trim(),
    ...(draft.apiKey.trim() ? { api_key: draft.apiKey.trim() } : {}),
    clear_api_key: draft.clearApiKey,
  });
  if (!result.success) {
    loadState.value = "error";
    requestError.value = result.error ?? t("ai.settings.saveFailed");
    toast.error(t("ai.settings.saveFailed"), { description: requestError.value });
    return;
  }
  const next = toDraft(result.data);
  Object.assign(draft, next);
  saved.value = cloneDraft(next);
  loadState.value = "idle";
  toast.success(t("ai.settings.saved"));
}

onMounted(() => {
  void load();
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div class="min-w-0">
        <p class="ui-label">{{ t("ai.settings.eyebrow") }}</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">{{ t("ai.settings.title") }}</h1>
        <p class="mt-2 text-sm text-muted-foreground">{{ t("ai.settings.description") }}</p>
      </div>
      <div class="flex w-full flex-wrap justify-end gap-2 sm:w-auto">
        <Button
          variant="outline"
          :disabled="loadState === 'loading' || loadState === 'saving'"
          @click="load(true)"
        >
          <RotateCcw class="size-4" :stroke-width="1.5" />
          {{ t("ai.settings.refresh") }}
        </Button>
        <Button :disabled="!canSave" @click="save">
          <Save class="size-4" :stroke-width="1.5" />
          {{ loadState === "saving" ? t("ai.settings.saving") : t("ai.settings.save") }}
        </Button>
      </div>
    </header>

    <div v-if="loadState === 'loading'" class="mt-6 grid gap-4">
      <Skeleton class="h-24 w-full" />
      <Skeleton class="h-56 w-full" />
    </div>

    <template v-else>
      <Alert v-if="requestError" class="mt-6 border-destructive/40">
        <AlertTitle>{{ t("ai.settings.loadFailed") }}</AlertTitle>
        <AlertDescription>{{ requestError }}</AlertDescription>
      </Alert>

      <form class="app-surface mt-6" @submit.prevent="save">
        <div class="flex items-center justify-between gap-4 border-b border-border px-5 py-4">
          <div class="min-w-0">
            <p class="text-sm font-medium">{{ t("ai.settings.enableTitle") }}</p>
            <p class="mt-1 text-xs text-muted-foreground">
              {{ t("ai.settings.enableDescription") }}
            </p>
          </div>
          <Switch :model-value="draft.enabled" @update:model-value="updateEnabled" />
        </div>

        <div class="grid gap-5 p-5">
          <Label class="grid gap-2">
            <span class="ui-label">{{ t("ai.settings.baseUrl") }}</span>
            <Input
              v-model="draft.baseUrl"
              autocomplete="url"
              :disabled="!draft.enabled"
              :placeholder="t('ai.settings.baseUrlPlaceholder')"
              @input="markDirty"
            />
            <span v-if="endpointError" class="text-xs text-destructive">{{ endpointError }}</span>
          </Label>

          <Label class="grid gap-2">
            <span class="ui-label">{{ t("ai.settings.model") }}</span>
            <Input
              v-model="draft.model"
              :disabled="!draft.enabled"
              :placeholder="t('ai.settings.modelPlaceholder')"
              @input="markDirty"
            />
            <span v-if="modelError" class="text-xs text-destructive">{{ modelError }}</span>
          </Label>

          <div class="grid gap-2">
            <Label for="ai-api-key" class="ui-label">{{ t("ai.settings.apiKey") }}</Label>
            <div class="flex gap-2">
              <div class="relative min-w-0 flex-1">
                <KeyRound
                  class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
                  :stroke-width="1.5"
                />
                <Input
                  id="ai-api-key"
                  :model-value="draft.apiKey"
                  class="pl-9"
                  :disabled="!draft.enabled"
                  :placeholder="
                    draft.apiKeyConfigured
                      ? t('ai.settings.apiKeyConfigured')
                      : t('ai.settings.apiKeyPlaceholder')
                  "
                  type="password"
                  autocomplete="new-password"
                  @update:model-value="updateApiKey"
                />
              </div>
              <Button
                v-if="draft.apiKeyConfigured"
                variant="outline"
                size="icon"
                :aria-label="t('ai.settings.clearApiKey')"
                :title="t('ai.settings.clearApiKey')"
                type="button"
                @click="clearApiKey"
              >
                <Trash2 class="size-4" :stroke-width="1.5" />
              </Button>
            </div>
            <span v-if="apiKeyError" class="text-xs text-destructive">{{ apiKeyError }}</span>
          </div>
        </div>

        <div class="flex flex-wrap justify-end gap-2 border-t border-border px-5 py-4">
          <Button
            variant="outline"
            type="button"
            :disabled="!isDirty || loadState === 'saving'"
            @click="reset"
          >
            <RotateCcw class="size-4" :stroke-width="1.5" />
            {{ t("ai.settings.reset") }}
          </Button>
          <Button type="submit" :disabled="!canSave">
            <Save class="size-4" :stroke-width="1.5" />
            {{ loadState === "saving" ? t("ai.settings.saving") : t("ai.settings.save") }}
          </Button>
        </div>
      </form>

      <Alert class="mt-4">
        <Bot class="size-4" :stroke-width="1.5" />
        <AlertTitle>{{ t("ai.settings.dataTitle") }}</AlertTitle>
        <AlertDescription>{{ t("ai.settings.dataDescription") }}</AlertDescription>
      </Alert>
    </template>
  </div>
</template>
