<script setup lang="ts">
import { RefreshCw } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { NotificationDelivery } from "@/lib/api";

defineProps<{
  deliveries: NotificationDelivery[];
  loading: boolean;
}>();

defineEmits<{
  refresh: [];
}>();

const { t } = useI18n();

function formatDeliveryDate(value: string) {
  return new Date(value).toLocaleString();
}

function deliveryStatusLabel(status: NotificationDelivery["status"]) {
  return t(`notifications.deliveryStatus.${status}`);
}
</script>

<template>
  <section
    class="app-surface mt-6 overflow-hidden"
    aria-labelledby="notification-deliveries-heading"
  >
    <header class="app-panel-header flex items-center justify-between gap-4 px-5 py-4">
      <div>
        <h2 id="notification-deliveries-heading" class="text-base font-medium">
          {{ t("notifications.deliveryHistory") }}
        </h2>
        <p class="mt-1 text-xs text-muted-foreground">
          {{ t("notifications.deliveryHistoryDescription") }}
        </p>
      </div>
      <Button
        variant="outline"
        size="icon-sm"
        type="button"
        :disabled="loading"
        :aria-label="t('notifications.refreshHistory')"
        @click="$emit('refresh')"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
      </Button>
    </header>
    <div v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" aria-live="polite">
      {{ t("notifications.loadingHistory") }}
    </div>
    <div v-else-if="deliveries.length === 0" class="px-5 py-8 text-sm text-muted-foreground">
      {{ t("notifications.emptyHistory") }}
    </div>
    <div v-else class="divide-y divide-border">
      <article
        v-for="delivery in deliveries"
        :key="delivery.id"
        class="grid gap-2 px-5 py-3 md:grid-cols-[minmax(0,1fr)_auto] md:items-center"
      >
        <div class="min-w-0">
          <div class="flex flex-wrap items-center gap-2">
            <span class="text-sm font-medium">{{ delivery.channel_name }}</span>
            <span class="font-mono text-[10px] text-muted-foreground">{{
              delivery.event_kind
            }}</span>
            <span
              class="rounded-[3px] border px-1.5 py-0.5 font-mono text-[9px] uppercase"
              :class="
                delivery.status === 'succeeded'
                  ? 'border-[var(--status-healthy)] text-[var(--status-healthy)]'
                  : delivery.status === 'failed'
                    ? 'border-destructive text-destructive'
                    : 'border-border text-muted-foreground'
              "
            >
              {{ deliveryStatusLabel(delivery.status) }}
            </span>
          </div>
          <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
            {{ delivery.source_kind }} / {{ delivery.source_id }}
          </p>
          <p
            v-if="delivery.correlation_id"
            class="mt-1 truncate font-mono text-[10px] text-muted-foreground"
            :title="delivery.correlation_id"
          >
            {{ t("notifications.correlationId") }} · {{ delivery.correlation_id }}
          </p>
          <p v-if="delivery.message" class="mt-1 text-[11px] text-muted-foreground">
            {{ delivery.message }}
            {{ t("notifications.deliveryAttempts", { count: delivery.attempt_count }) }}
          </p>
        </div>
        <time class="font-mono text-[10px] text-muted-foreground" :datetime="delivery.created_at">
          {{ formatDeliveryDate(delivery.created_at) }}
        </time>
      </article>
    </div>
  </section>
</template>
