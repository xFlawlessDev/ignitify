<script setup lang="ts">
import { CircleCheck, CircleX, ExternalLink, Globe2, RefreshCw, Trash2 } from "@lucide/vue";
import type { DnsValidationState } from "./DomainConfigurationPanel.vue";
import type { DomainSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  canManage: boolean;
  domains: DomainSummary[];
  services: ServiceSummary[];
  httpsEnabled: boolean;
  dnsStates: Record<string, DnsValidationState>;
  dnsMessages: Record<string, string>;
}>();

const emit = defineEmits<{
  validateDns: [domain: DomainSummary];
  remove: [domain: DomainSummary];
}>();

function serviceName(id: string) {
  return props.services.find((service) => service.id === id)?.name ?? "Unknown service";
}

function servicePort(id: string) {
  return props.services.find((service) => service.id === id)?.internal_port ?? "-";
}

function domainUrl(hostname: string) {
  return `${props.httpsEnabled ? "https" : "http"}://${hostname}`;
}

function statusLabel(status: DomainSummary["status"]) {
  return status === "active" ? "Active" : status === "failed" ? "Failed" : "Pending";
}
</script>

<template>
  <section class="border border-border bg-card" aria-labelledby="managed-domains-heading">
    <header class="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
      <div>
        <p class="ui-label">Managed routes</p>
        <h2 id="managed-domains-heading" class="mt-1.5 text-base font-medium">Project domains</h2>
        <p class="mt-1.5 text-xs leading-5 text-muted-foreground">
          Verify DNS before sharing a route. Links open the public endpoint in a new tab.
        </p>
      </div>
      <span class="shrink-0 font-mono text-[10px] text-muted-foreground"
        >{{ props.domains.length }} route{{ props.domains.length === 1 ? "" : "s" }}</span
      >
    </header>

    <div v-if="props.domains.length" class="divide-y divide-border">
      <article
        v-for="domain in props.domains"
        :key="domain.id"
        class="grid gap-4 px-5 py-4 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center"
      >
        <div class="grid min-w-0 gap-2">
          <div class="flex min-w-0 items-center gap-2">
            <span
              class="grid size-7 shrink-0 place-items-center border border-border bg-muted text-muted-foreground"
            >
              <Globe2 class="size-3.5" :stroke-width="1.5" />
            </span>
            <a
              class="truncate font-mono text-sm font-medium hover:underline"
              :href="domainUrl(domain.hostname)"
              target="_blank"
              rel="noreferrer"
            >
              {{ domain.hostname }}
            </a>
          </div>
          <div
            class="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
          >
            <span>{{ serviceName(domain.service_id) }}</span>
            <span class="font-mono">port {{ servicePort(domain.service_id) }}</span>
            <span
              class="font-medium"
              :class="
                domain.status === 'active'
                  ? 'text-metric-green'
                  : domain.status === 'failed'
                    ? 'text-destructive'
                    : 'text-muted-foreground'
              "
            >
              {{ statusLabel(domain.status) }}
            </span>
          </div>
          <span v-if="domain.last_error" class="text-xs text-destructive">{{
            domain.last_error
          }}</span>
          <p
            v-if="props.dnsStates[domain.id] && props.dnsStates[domain.id] !== 'idle'"
            class="flex items-center gap-1.5 text-[11px]"
            :class="
              props.dnsStates[domain.id] === 'valid'
                ? 'text-metric-green'
                : props.dnsStates[domain.id] === 'checking'
                  ? 'text-muted-foreground'
                  : 'text-destructive'
            "
            role="status"
            aria-live="polite"
          >
            <CircleCheck
              v-if="props.dnsStates[domain.id] === 'valid'"
              class="size-3.5"
              :stroke-width="1.7"
            />
            <CircleX
              v-else-if="props.dnsStates[domain.id] !== 'checking'"
              class="size-3.5"
              :stroke-width="1.7"
            />
            <RefreshCw v-else class="size-3.5 animate-spin" :stroke-width="1.7" />
            {{ props.dnsMessages[domain.id] }}
          </p>
        </div>

        <div class="flex flex-wrap items-center gap-2 lg:justify-end">
          <button
            class="inline-flex h-8 items-center gap-1.5 rounded-[3px] border border-border px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
            type="button"
            :disabled="props.dnsStates[domain.id] === 'checking'"
            :aria-label="`Validate DNS for ${domain.hostname}`"
            title="Validate DNS"
            @click="emit('validateDns', domain)"
          >
            <RefreshCw
              class="size-3.5"
              :class="props.dnsStates[domain.id] === 'checking' ? 'animate-spin' : ''"
              :stroke-width="1.5"
            />
            Validate DNS
          </button>
          <a
            class="inline-flex h-8 items-center gap-1.5 rounded-[3px] border border-border px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            :href="domainUrl(domain.hostname)"
            :aria-label="`Open ${domain.hostname}`"
            title="Open link"
            target="_blank"
            rel="noreferrer"
          >
            <ExternalLink class="size-3.5" :stroke-width="1.5" />
            Open link
          </a>
          <button
            v-if="props.canManage"
            class="grid size-8 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-destructive/40 hover:bg-destructive/10 hover:text-destructive"
            type="button"
            :aria-label="`Remove ${domain.hostname}`"
            title="Remove domain"
            @click="emit('remove', domain)"
          >
            <Trash2 class="size-4" :stroke-width="1.5" />
          </button>
        </div>
      </article>
    </div>
    <div v-else class="px-5 py-8">
      <p class="text-sm font-medium">No managed domains</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Add a public hostname above to create the first route for this project.
      </p>
    </div>
  </section>
</template>
