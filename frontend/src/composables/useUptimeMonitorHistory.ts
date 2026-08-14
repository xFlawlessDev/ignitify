import { shallowRef, triggerRef } from "vue";
import { apiGetUptimeMonitorHistory, type UptimeMonitorHistory } from "@/lib/api/uptime-monitors";

const histories = shallowRef<Record<string, UptimeMonitorHistory>>({});
const loadingMonitorId = shallowRef<string | null>(null);
const error = shallowRef<string | null>(null);

export function useUptimeMonitorHistory() {
  async function loadHistory(
    monitorId: string,
    windowHours = 24,
  ): Promise<UptimeMonitorHistory | null> {
    loadingMonitorId.value = monitorId;
    error.value = null;
    try {
      const result = await apiGetUptimeMonitorHistory(monitorId, {
        hours: windowHours,
        limit: 500,
      });
      if (!result.success) {
        error.value = result.error ?? "Unable to load monitor history.";
        return null;
      }
      histories.value[monitorId] = result.data;
      triggerRef(histories);
      return result.data;
    } finally {
      if (loadingMonitorId.value === monitorId) loadingMonitorId.value = null;
    }
  }

  function clearHistory(monitorId: string) {
    if (!(monitorId in histories.value)) return;
    delete histories.value[monitorId];
    triggerRef(histories);
  }

  return {
    histories,
    loadingMonitorId,
    error,
    loadHistory,
    clearHistory,
  };
}
