<script setup lang="ts">
import { CircleAlert, ShieldCheck } from "@lucide/vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type { SupplyChainCheck, SupplyChainReport } from "@/lib/types";

const props = defineProps<{
  report: SupplyChainReport | null;
}>();

const { t } = useI18n();
const checks = computed(() =>
  props.report
    ? [
        { key: "provenance", label: t("supplyChain.provenance"), value: props.report.provenance },
        { key: "sbom", label: t("supplyChain.sbom"), value: props.report.sbom },
        {
          key: "vulnerabilities",
          label: t("supplyChain.vulnerabilities"),
          value: props.report.vulnerabilities,
        },
      ]
    : [],
);

function statusClass(check: Pick<SupplyChainCheck, "status">) {
  return check.status === "pass"
    ? "border-[var(--status-healthy)]/40 bg-[var(--status-healthy)]/10 text-[var(--status-healthy)]"
    : "border-[var(--status-live)]/40 bg-[var(--status-live)]/10 text-[var(--status-live)]";
}

function statusLabel(check: Pick<SupplyChainCheck, "status">) {
  return check.status === "pass" ? t("supplyChain.pass") : t("supplyChain.warning");
}
</script>

<template>
  <section v-if="report" class="border-y border-border py-4" :aria-label="t('supplyChain.title')">
    <div class="flex flex-wrap items-start justify-between gap-3 px-1">
      <div>
        <p class="ui-label">{{ t("supplyChain.title") }}</p>
        <p class="mt-1 text-xs text-muted-foreground">{{ t("supplyChain.warningMode") }}</p>
      </div>
      <Badge
        variant="outline"
        class="gap-1.5 rounded-[4px] font-normal"
        :class="statusClass(report)"
      >
        <CircleAlert v-if="report.status === 'warning'" class="size-3" :stroke-width="1.5" />
        <ShieldCheck v-else class="size-3" :stroke-width="1.5" />
        {{ statusLabel(report) }}
      </Badge>
    </div>

    <div class="mt-4 divide-y divide-border border-t border-border">
      <div
        v-for="check in checks"
        :key="check.key"
        class="grid gap-2 px-1 py-3 sm:grid-cols-[9rem_1fr_auto] sm:items-start"
      >
        <p class="text-xs font-medium">{{ check.label }}</p>
        <div class="min-w-0">
          <p class="text-xs text-muted-foreground">{{ check.value.summary }}</p>
          <p v-if="check.value.remediation" class="mt-1 text-xs text-muted-foreground">
            {{ check.value.remediation }}
          </p>
        </div>
        <Badge
          variant="outline"
          class="w-fit rounded-[4px] text-[10px]"
          :class="statusClass(check.value)"
        >
          {{ statusLabel(check.value) }}
        </Badge>
      </div>
    </div>
  </section>
</template>
