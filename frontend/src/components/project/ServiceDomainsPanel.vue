<script setup lang="ts">
import { shallowRef, watch, computed } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import DomainConfigurationPanel from "./DomainConfigurationPanel.vue";
import DomainRouteList from "./DomainRouteList.vue";
import type { DomainSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  canManage: boolean;
  domains: DomainSummary[];
  error: string | null;
  loading: boolean;
  services: ServiceSummary[];
}>();

const emit = defineEmits<{
  create: [serviceId: string, hostname: string];
  remove: [domain: DomainSummary];
  retry: [];
  verify: [domain: DomainSummary];
}>();

const hostname = shallowRef("");
const confirmHostname = shallowRef("");
const serviceId = shallowRef("");
const confirmation = shallowRef<DomainSummary | null>(null);

const routableServices = computed(() => props.services.filter((service) => service.internal_port));
const domainError = computed(() => {
  const value = hostname.value.trim();
  if (!value) return "Custom domain is required.";
  if (
    value.length > 253 ||
    value.includes("..") ||
    !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(value) ||
    value
      .split(".")
      .some((label) => label.length > 63 || label.startsWith("-") || label.endsWith("-"))
  ) {
    return "Use a valid hostname without a protocol or path.";
  }
  return "";
});

watch(
  routableServices,
  (services) => {
    if (!services.some((service) => service.id === serviceId.value)) {
      serviceId.value = services[0]?.id ?? "";
    }
  },
  { immediate: true },
);

function submit() {
  if (!serviceId.value || domainError.value) return;
  emit("create", serviceId.value, hostname.value.trim());
  hostname.value = "";
}

function requestRemove(domain: DomainSummary) {
  confirmation.value = domain;
  confirmHostname.value = "";
}

function removeConfirmed() {
  if (!confirmation.value) return;
  emit("remove", confirmation.value);
  confirmation.value = null;
  confirmHostname.value = "";
}
</script>

<template>
  <div class="grid gap-4">
    <DomainConfigurationPanel
      v-if="canManage"
      :services="services"
      :server-domain="hostname"
      :service-id="serviceId"
      :domain-error="domainError"
      @update:server-domain="hostname = $event"
      @update:service-id="serviceId = $event"
      @create="submit"
    />

    <div v-if="loading" class="app-surface px-5" role="status" aria-label="Loading domains">
      <div
        v-for="index in 3"
        :key="index"
        class="grid gap-3 border-b border-border py-4 last:border-b-0 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center"
      >
        <div class="grid min-w-0 gap-2">
          <Skeleton class="h-3 w-40 max-w-full" />
          <Skeleton class="h-2.5 w-28 max-w-full" />
        </div>
        <Skeleton class="h-8 w-24" />
      </div>
    </div>
    <section v-else-if="error && !domains.length" class="app-surface px-5 py-5" role="alert">
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
    </section>
    <p
      v-else-if="error"
      class="rounded-[10px] border border-destructive/40 bg-card px-5 py-3 text-xs text-destructive"
      role="alert"
    >
      {{ error }}
    </p>
    <DomainRouteList
      v-else-if="!error || domains.length"
      :can-manage="canManage"
      :domains="domains"
      :services="services"
      @verify="(domain) => emit('verify', domain)"
      @remove="requestRemove"
    />

    <div
      v-if="confirmation"
      class="app-surface p-5"
      role="alertdialog"
      aria-labelledby="domain-confirm-title"
    >
      <p id="domain-confirm-title" class="text-sm font-medium">
        Remove {{ confirmation.hostname }}?
      </p>
      <p class="mt-1 text-xs text-muted-foreground">Type hostname to confirm route removal.</p>
      <Input
        v-model="confirmHostname"
        class="mt-3"
        :placeholder="confirmation.hostname"
        autocomplete="off"
      />
      <div class="mt-3 flex gap-2">
        <Button
          size="sm"
          variant="destructive"
          :disabled="confirmHostname !== confirmation.hostname"
          @click="removeConfirmed"
          >Remove</Button
        >
        <Button
          size="sm"
          variant="outline"
          @click="
            confirmation = null;
            confirmHostname = '';
          "
          >Cancel</Button
        >
      </div>
    </div>
  </div>
</template>
