<script setup lang="ts">
import {
  Activity,
  Box,
  CloudBackup,
  Globe2,
  HardDriveDownload,
  RefreshCw,
  Server,
} from "@lucide/vue";
import { computed, onMounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { apiGetOperationalHealthSummary } from "@/lib/api";
import type { OperationalHealthStatus, OperationalHealthSummary } from "@/lib/types";

interface HealthItem {
  label: string;
  status: OperationalHealthStatus | undefined;
  detail: string;
  icon: typeof Activity;
}

const summary = shallowRef<OperationalHealthSummary | null>(null);
const state = shallowRef<"loading" | "idle" | "error">("loading");
const requestError = shallowRef("");
const { t } = useI18n();

const items = computed<HealthItem[]>(() => {
  const value = summary.value;
  return [
    {
      label: t("operationalHealth.deployments"),
      status: value?.deployments.status,
      detail: value
        ? t("operationalHealth.deploymentDetail", {
            active: value.deployments.active_count,
            queued: value.deployments.queued_count,
            failed: value.deployments.failed_retry_count,
            retries: value.deployments.retry_count,
          })
        : "",
      icon: Activity,
    },
    {
      label: t("operationalHealth.backup"),
      status: value?.backup.status,
      detail: backupDetail(value),
      icon: HardDriveDownload,
    },
    {
      label: t("operationalHealth.ingress"),
      status: value?.ingress.status,
      detail: value
        ? t("operationalHealth.domainDetail", {
            active: value.domains.active_count,
            pending: value.domains.pending_count,
            failed: value.domains.failed_count,
          })
        : "",
      icon: Globe2,
    },
    {
      label: t("operationalHealth.certificates"),
      status: value?.certificates.status,
      detail: value
        ? t("operationalHealth.certificateDetail", {
            provider: value.certificates.provider,
            count: value.certificates.stored_certificate_count,
          })
        : "",
      icon: CloudBackup,
    },
    {
      label: t("operationalHealth.remoteHosts"),
      status: value?.remote_agents.status,
      detail: remoteDetail(value),
      icon: Server,
    },
    {
      label: t("operationalHealth.controlPlane"),
      status: value?.control_plane.status,
      detail: value ? readinessDetail(value) : "",
      icon: Box,
    },
  ];
});

function backupDetail(value: OperationalHealthSummary | null) {
  if (!value) return "";
  if (!value.backup.configured) return t("operationalHealth.notConfigured");
  if (!value.backup.enabled) return t("operationalHealth.disabled");
  if (!value.backup.schedule_interval_hours) return t("operationalHealth.scheduleDisabled");
  return value.backup.latest_age_seconds === null
    ? t("operationalHealth.noScheduledRun")
    : t("operationalHealth.lastRun", { age: formatAge(value.backup.latest_age_seconds) });
}

function remoteDetail(value: OperationalHealthSummary | null) {
  if (!value) return "";
  if (!value.remote_agents.server_count) return t("operationalHealth.notConfigured");
  const heartbeat = value.remote_agents.oldest_heartbeat_age_seconds;
  return t("operationalHealth.remoteDetail", {
    online: value.remote_agents.online_count,
    offline: value.remote_agents.offline_count,
    oldest: heartbeat === null ? "-" : formatAge(heartbeat),
  });
}

function readinessDetail(value: OperationalHealthSummary) {
  return t("operationalHealth.readinessDetail", {
    runtime: value.runtime.status,
    worker: value.worker.status,
    ingress: value.ingress.status,
  });
}

function formatAge(seconds: number) {
  if (seconds < 60) return `${seconds}s`;
  if (seconds < 3_600) return `${Math.floor(seconds / 60)}m`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h`;
  return `${Math.floor(seconds / 86_400)}d`;
}

function statusClass(status: OperationalHealthStatus | undefined) {
  if (status === "failed" || status === "stalled" || status === "unavailable") {
    return "text-destructive";
  }
  if (status === "healthy" || status === "ready") return "text-metric-green";
  if (status === "active" || status === "running") return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

async function load() {
  state.value = "loading";
  requestError.value = "";
  const result = await apiGetOperationalHealthSummary();
  if (!result.success) {
    summary.value = null;
    requestError.value = result.error ?? t("operationalHealth.loadFailed");
    state.value = "error";
    return;
  }
  summary.value = result.data;
  state.value = "idle";
}

onMounted(() => {
  void load();
});
</script>

<template>
  <section class="app-surface" aria-labelledby="operational-health-heading">
    <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
      <div>
        <p class="ui-label">{{ t("operationalHealth.eyebrow") }}</p>
        <h2 id="operational-health-heading" class="mt-1.5 text-base font-medium">
          {{ t("operationalHealth.title") }}
        </h2>
      </div>
      <Button
        variant="ghost"
        size="icon"
        type="button"
        :disabled="state === 'loading'"
        :aria-label="t('operationalHealth.refresh')"
        :title="t('operationalHealth.refresh')"
        @click="load"
      >
        <RefreshCw
          class="size-4"
          :class="state === 'loading' ? 'animate-spin' : ''"
          :stroke-width="1.5"
        />
      </Button>
    </header>

    <p
      v-if="requestError"
      class="border-t border-border px-5 py-3 text-[11px] text-destructive"
      role="alert"
    >
      {{ requestError }}
    </p>
    <div
      v-else-if="state === 'loading'"
      class="border-t border-border px-5 py-4 text-xs text-muted-foreground"
    >
      {{ t("operationalHealth.loading") }}
    </div>
    <dl
      v-else
      class="grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-3"
    >
      <div v-for="item in items" :key="item.label" class="flex min-w-0 gap-3 px-5 py-3.5">
        <component
          :is="item.icon"
          class="mt-0.5 size-4 shrink-0 text-muted-foreground"
          :stroke-width="1.5"
        />
        <div class="min-w-0">
          <dt class="text-xs font-medium">{{ item.label }}</dt>
          <dd class="mt-0.5 font-mono text-[11px]" :class="statusClass(item.status)">
            {{ item.status?.replaceAll("_", " ") }}
          </dd>
          <dd class="mt-1 text-[11px] leading-4 text-muted-foreground">{{ item.detail }}</dd>
        </div>
      </div>
    </dl>
  </section>
</template>
