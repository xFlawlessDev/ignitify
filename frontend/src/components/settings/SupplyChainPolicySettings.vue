<script setup lang="ts">
import { ShieldAlert } from "@lucide/vue";
import { computed, onMounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { apiGetSupplyChainPolicy, apiUpdateSupplyChainPolicy } from "@/lib/api";
import type { SupplyChainEnforcement } from "@/lib/types";

const { t } = useI18n();
const enforcement = shallowRef<SupplyChainEnforcement>("warning");
const savedEnforcement = shallowRef<SupplyChainEnforcement | null>(null);
const updatedAt = shallowRef("");
const state = shallowRef<"loading" | "idle" | "saving" | "error">("loading");
const requestError = shallowRef("");

const isDirty = computed(
  () => savedEnforcement.value !== null && enforcement.value !== savedEnforcement.value,
);
const canSave = computed(
  () => state.value !== "loading" && state.value !== "saving" && isDirty.value,
);

function selectEnforcement(value: string) {
  if (value === "warning" || value === "require-provenance") {
    enforcement.value = value;
    requestError.value = "";
  }
}

async function loadPolicy() {
  state.value = "loading";
  requestError.value = "";
  const result = await apiGetSupplyChainPolicy();
  if (!result.success) {
    requestError.value = result.error ?? t("supplyChainSettings.loadError");
    state.value = "error";
    return;
  }
  enforcement.value = result.data.enforcement;
  savedEnforcement.value = result.data.enforcement;
  updatedAt.value = result.data.updated_at;
  state.value = "idle";
}

async function savePolicy() {
  if (!canSave.value) return;
  state.value = "saving";
  requestError.value = "";
  const result = await apiUpdateSupplyChainPolicy(enforcement.value);
  if (!result.success) {
    requestError.value = result.error ?? t("supplyChainSettings.saveError");
    state.value = "error";
    return;
  }
  savedEnforcement.value = result.data.enforcement;
  updatedAt.value = result.data.updated_at;
  state.value = "idle";
  toast.success(t("supplyChainSettings.saved"));
}

onMounted(loadPolicy);
</script>

<template>
  <section class="app-surface" aria-labelledby="supply-chain-policy-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <ShieldAlert class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">{{ t("supplyChainSettings.eyebrow") }}</p>
        <h2 id="supply-chain-policy-heading" class="mt-1.5 text-base font-medium">
          {{ t("supplyChainSettings.title") }}
        </h2>
        <p class="mt-1.5 max-w-[62ch] text-xs leading-5 text-muted-foreground">
          {{ t("supplyChainSettings.description") }}
        </p>
      </div>
    </header>

    <div class="grid gap-2 border-t border-border px-5 py-4 sm:max-w-xl">
      <label for="supply-chain-enforcement" class="text-xs font-medium">
        {{ t("supplyChainSettings.enforcement") }}
      </label>
      <Select :model-value="enforcement" @update:model-value="selectEnforcement(String($event))">
        <SelectTrigger id="supply-chain-enforcement" class="rounded-[3px] text-sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="warning">{{ t("supplyChainSettings.warning") }}</SelectItem>
          <SelectItem value="require-provenance">
            {{ t("supplyChainSettings.requireProvenance") }}
          </SelectItem>
        </SelectContent>
      </Select>
      <p class="text-[11px] leading-4 text-muted-foreground">
        {{
          enforcement === "require-provenance"
            ? t("supplyChainSettings.requireProvenanceDescription")
            : t("supplyChainSettings.warningDescription")
        }}
      </p>
      <p v-if="updatedAt" class="font-mono text-[10px] text-muted-foreground">
        {{ t("supplyChainSettings.updated", { time: updatedAt }) }}
      </p>
      <p v-if="requestError" class="text-[11px] text-destructive" role="alert">
        {{ requestError }}
      </p>
      <div class="mt-2 flex items-center gap-3">
        <Button size="sm" type="button" :disabled="!canSave" @click="savePolicy">
          {{ t("supplyChainSettings.save") }}
        </Button>
        <span v-if="state === 'loading'" class="text-[11px] text-muted-foreground">
          {{ t("supplyChainSettings.loading") }}
        </span>
      </div>
    </div>
  </section>
</template>
