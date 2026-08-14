<script setup lang="ts">
import { CheckCircle2, Clock3, ShieldCheck } from "@lucide/vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DeploymentApproval, DeploymentSourceIdentity } from "@/lib/types";

const props = defineProps<{
  approval: DeploymentApproval;
  identity?: DeploymentSourceIdentity;
  canApprove: boolean;
  submitting: boolean;
}>();

defineEmits<{ approve: [] }>();

const { t } = useI18n();
const isPending = computed(() => props.approval.status === "pending");
const isApproved = computed(() => props.approval.status === "approved");

function formatTimestamp(value?: string) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
</script>

<template>
  <section
    v-if="approval.status !== 'not_required'"
    class="border-y border-border py-4"
    :aria-label="t('deploymentApproval.title')"
  >
    <div class="flex flex-wrap items-start justify-between gap-3 px-1">
      <div>
        <p class="ui-label">{{ t("deploymentApproval.title") }}</p>
        <p class="mt-1 text-xs text-muted-foreground">
          {{
            isPending
              ? t("deploymentApproval.pendingDescription")
              : t("deploymentApproval.approvedDescription")
          }}
        </p>
      </div>
      <Badge
        variant="outline"
        class="gap-1.5 rounded-[4px] font-normal"
        :class="
          isPending
            ? 'border-[var(--status-live)]/40 bg-[var(--status-live)]/10 text-[var(--status-live)]'
            : 'border-[var(--status-healthy)]/40 bg-[var(--status-healthy)]/10 text-[var(--status-healthy)]'
        "
      >
        <Clock3 v-if="isPending" class="size-3" :stroke-width="1.5" />
        <CheckCircle2 v-else class="size-3" :stroke-width="1.5" />
        {{ isPending ? t("deploymentApproval.pending") : t("deploymentApproval.approved") }}
      </Badge>
    </div>

    <Alert v-if="isPending" class="mt-4 border-[var(--status-live)]/40">
      <ShieldCheck :stroke-width="1.5" />
      <AlertTitle>{{ t("deploymentApproval.actionRequired") }}</AlertTitle>
      <AlertDescription class="flex flex-wrap items-center justify-between gap-3">
        <span>{{
          t("deploymentApproval.requested", { time: formatTimestamp(approval.requested_at) })
        }}</span>
        <Button v-if="canApprove" size="sm" :disabled="submitting" @click="$emit('approve')">
          <ShieldCheck data-icon="inline-start" :stroke-width="1.5" />
          {{ t("deploymentApproval.approve") }}
        </Button>
        <span v-else class="text-xs">{{ t("deploymentApproval.ownerRequired") }}</span>
      </AlertDescription>
    </Alert>
    <p v-else-if="isApproved" class="mt-4 px-1 text-xs text-muted-foreground">
      {{ t("deploymentApproval.approvedAt", { time: formatTimestamp(approval.approved_at) }) }}
    </p>
    <div v-if="identity" class="mt-4 grid gap-2 border-t border-border px-1 pt-3 text-xs">
      <p class="text-muted-foreground">
        {{ t("deploymentApproval.sourceRevision") }}
        <code class="ml-1 break-all font-mono text-foreground">
          {{ identity.source_revision ?? t("deploymentApproval.resolvesAfterApproval") }}
        </code>
      </p>
      <p class="text-muted-foreground">
        {{ t("deploymentApproval.imageDigest") }}
        <code class="ml-1 break-all font-mono text-foreground">
          {{ identity.image_digest ?? t("deploymentApproval.resolvesAfterApproval") }}
        </code>
      </p>
    </div>
  </section>
</template>
