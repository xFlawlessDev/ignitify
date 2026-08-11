<script setup lang="ts">
import { ShieldCheck } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

interface Props {
  domain: string;
  error?: string;
}

defineProps<Props>();
const emit = defineEmits<{
  (event: "update:domain", value: string): void;
}>();
const { t } = useI18n();
</script>

<template>
  <section class="app-surface" aria-labelledby="control-plane-ingress-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <ShieldCheck class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">{{ t("controlPlaneIngress.eyebrow") }}</p>
        <h2 id="control-plane-ingress-heading" class="mt-1.5 text-base font-medium">
          {{ t("controlPlaneIngress.title") }}
        </h2>
      </div>
    </header>

    <div class="grid gap-2 px-5 py-5">
      <Label for="control-plane-domain" class="text-xs font-medium">
        {{ t("controlPlaneIngress.domain") }}
      </Label>
      <Input
        id="control-plane-domain"
        :model-value="domain"
        class="rounded-[3px] font-mono text-sm"
        :placeholder="t('controlPlaneIngress.placeholder')"
        autocomplete="off"
        spellcheck="false"
        :aria-invalid="Boolean(error)"
        aria-describedby="control-plane-domain-help control-plane-domain-error"
        @update:model-value="emit('update:domain', String($event))"
      />
      <p id="control-plane-domain-help" class="text-[11px] leading-4 text-muted-foreground">
        {{ t("controlPlaneIngress.help") }}
      </p>
      <p v-if="error" id="control-plane-domain-error" class="text-[11px] text-destructive">
        {{ error }}
      </p>
    </div>
  </section>
</template>
