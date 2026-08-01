<script setup lang="ts">
import { ExternalLink, Globe2, Trash2 } from "@lucide/vue";
import { shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
}>();

const hostname = shallowRef("");
const confirmHostname = shallowRef("");
const serviceId = shallowRef("");
const confirmation = shallowRef<DomainSummary | null>(null);

function submit() {
  if (!serviceId.value || !hostname.value.trim()) return;
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

function serviceName(id: string) {
  return props.services.find((service) => service.id === id)?.name ?? "Unknown service";
}
</script>

<template>
  <section class="border border-border bg-card">
    <div class="flex items-end justify-between gap-4 border-b border-border px-5 pt-5 pb-4">
      <div>
        <p class="ui-label">Ingress</p>
        <h2 class="mt-2 text-xl leading-none font-normal">Domains</h2>
      </div>
    </div>

    <form
      v-if="canManage && services.length"
      class="grid gap-3 border-b border-border p-5 sm:grid-cols-[minmax(0,1fr)_180px_auto]"
      @submit.prevent="submit"
    >
      <label class="grid gap-1 text-xs text-muted-foreground">
        Hostname
        <Input v-model="hostname" autocomplete="off" placeholder="app.example.com" />
      </label>
      <label class="grid gap-1 text-xs text-muted-foreground">
        Service
        <select v-model="serviceId" class="h-9 border border-input bg-background px-3 text-sm">
          <option value="" disabled>Select service</option>
          <option v-for="service in services" :key="service.id" :value="service.id">
            {{ service.name }}
          </option>
        </select>
      </label>
      <Button class="self-end" size="sm" type="submit">Add domain</Button>
    </form>

    <p v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" role="status">
      Loading domains...
    </p>
    <section v-else-if="error && !domains.length" class="px-5 py-5" role="alert">
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button class="mt-3" size="sm" variant="outline" @click="emit('retry')">Retry</Button>
    </section>
    <div v-else-if="domains.length" class="divide-y divide-border">
      <p v-if="error" class="px-5 py-3 text-xs text-destructive" role="alert">{{ error }}</p>
      <div
        v-for="domain in domains"
        :key="domain.id"
        class="flex items-center justify-between gap-4 px-5 py-3"
      >
        <div class="grid min-w-0 gap-1">
          <span class="flex items-center gap-2 text-sm font-medium"
            ><Globe2 class="size-4 text-muted-foreground" :stroke-width="1.5" />{{
              domain.hostname
            }}</span
          >
          <span class="text-xs text-muted-foreground"
            >{{ serviceName(domain.service_id) }} · {{ domain.status }}</span
          >
          <span v-if="domain.last_error" class="text-xs text-destructive">{{
            domain.last_error
          }}</span>
        </div>
        <div class="flex shrink-0 items-center gap-2">
          <a
            v-if="domain.status === 'active'"
            class="grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground"
            :href="`https://${domain.hostname}`"
            :aria-label="`Open ${domain.hostname}`"
            title="Open HTTPS URL"
            target="_blank"
            rel="noreferrer"
            ><ExternalLink class="size-4" :stroke-width="1.5"
          /></a>
          <button
            v-if="canManage"
            class="grid size-8 place-items-center rounded-md text-muted-foreground hover:bg-muted hover:text-destructive"
            type="button"
            :aria-label="`Remove ${domain.hostname}`"
            title="Remove domain"
            @click="requestRemove(domain)"
          >
            <Trash2 class="size-4" :stroke-width="1.5" />
          </button>
        </div>
      </div>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No managed domains</p>
      <p class="mt-1 text-xs text-muted-foreground">
        HTTPS appears after route reconciliation. Traefik details stay operator-only.
      </p>
    </div>
    <div
      v-if="confirmation"
      class="border-t border-border p-5"
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
  </section>
</template>
