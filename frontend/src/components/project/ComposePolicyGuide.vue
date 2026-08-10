<script setup lang="ts">
import { Info } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";

defineProps<{ gitSource?: boolean }>();

const { t } = useI18n();
</script>

<template>
  <Alert variant="default" class="border-border bg-muted/20">
    <Info :stroke-width="1.5" />
    <AlertTitle>{{ t("composeGuide.title") }}</AlertTitle>
    <AlertDescription class="mt-2 grid gap-2 text-xs leading-5 text-muted-foreground">
      <ul class="grid gap-1.5">
        <li>
          {{ t("composeGuide.imagePrefix") }}
          <span class="font-mono text-foreground">@sha256</span> {{ t("composeGuide.imageSuffix") }}
        </li>
        <li>
          {{ t("composeGuide.ingressPrefix") }}
          <span class="font-mono text-foreground">ports</span>
          {{ t("composeGuide.ingressSuffix") }}
        </li>
        <li>
          {{ t("composeGuide.applicationPrefix") }}
          <strong class="font-medium text-foreground">Application</strong>
          {{ t("composeGuide.applicationSuffix") }}
          <span class="font-mono text-foreground">build</span>.
        </li>
        <li>
          {{ t("composeGuide.environmentPrefix") }}
          <span class="font-mono text-foreground">env_file</span>.
        </li>
      </ul>
      <p v-if="gitSource">
        {{ t("composeGuide.gitIngress") }}
      </p>
    </AlertDescription>
  </Alert>
</template>
