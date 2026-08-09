<script setup lang="ts">
import { CircleCheck, CircleX, ExternalLink, Globe2, RefreshCw, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import type { DomainSummary, ServiceSummary } from "@/lib/types";

const props = defineProps<{
  canManage: boolean;
  domains: DomainSummary[];
  services: ServiceSummary[];
}>();

const emit = defineEmits<{
  verify: [domain: DomainSummary];
  remove: [domain: DomainSummary];
}>();

function serviceName(id: string) {
  return props.services.find((service) => service.id === id)?.name ?? "Unknown service";
}

function servicePort(id: string) {
  return props.services.find((service) => service.id === id)?.internal_port ?? "-";
}

function domainUrl(hostname: string) {
  return `https://${hostname}`;
}

function statusLabel(status: DomainSummary["status"]) {
  return status === "active" ? "Active" : status === "failed" ? "Failed" : "Pending";
}

function dnsStatusLabel(status: DomainSummary["dns_status"]) {
  return status === "valid"
    ? "DNS verified"
    : status === "missing"
      ? "DNS record not found"
      : status === "unavailable"
        ? "DNS lookup unavailable"
        : status === "pending"
          ? "DNS verification pending"
          : "DNS not verified";
}

function recordInstruction(domain: DomainSummary) {
  if (!domain.dns_record_type || !domain.dns_record_target) return null;
  return `${domain.dns_record_type.toUpperCase()} ${domain.hostname} -> ${domain.dns_record_target}`;
}

function formatCheckedAt(value: string | null) {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
</script>

<template>
  <section class="app-surface" aria-labelledby="managed-domains-heading">
    <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
      <div>
        <p class="ui-label">Managed routes</p>
        <h2 id="managed-domains-heading" class="mt-1.5 text-base font-medium">Project domains</h2>
        <p class="mt-1.5 text-xs leading-5 text-muted-foreground">
          Add the DNS record shown below at your DNS provider, then verify it here.
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
          <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px]">
            <span class="font-mono text-muted-foreground">
              {{ domain.dns_record_type?.toUpperCase() ?? "DNS" }}
              {{ domain.dns_record_target ?? "Configure target in Infrastructure" }}
            </span>
            <span
              class="inline-flex items-center gap-1 font-medium"
              :class="
                domain.dns_status === 'valid'
                  ? 'text-metric-green'
                  : domain.dns_status === 'pending'
                    ? 'text-muted-foreground'
                    : domain.dns_status === 'not_checked'
                      ? 'text-muted-foreground'
                      : 'text-destructive'
              "
              role="status"
              aria-live="polite"
            >
              <CircleCheck
                v-if="domain.dns_status === 'valid'"
                class="size-3.5"
                :stroke-width="1.7"
              />
              <RefreshCw
                v-else-if="domain.dns_status === 'pending'"
                class="size-3.5 animate-spin"
                :stroke-width="1.7"
              />
              <CircleX
                v-else-if="domain.dns_status !== 'not_checked'"
                class="size-3.5"
                :stroke-width="1.7"
              />
              {{ dnsStatusLabel(domain.dns_status) }}
            </span>
          </div>
          <span v-if="domain.dns_error" class="text-xs text-destructive">{{
            domain.dns_error
          }}</span>
          <p
            v-if="recordInstruction(domain) && domain.dns_status !== 'valid'"
            class="text-[11px] leading-4 text-muted-foreground"
          >
            Required record:
            <code class="font-mono text-foreground">{{ recordInstruction(domain) }}</code>
          </p>
          <p
            v-if="domain.dns_status === 'missing'"
            class="text-[11px] leading-4 text-muted-foreground"
          >
            DNS propagation can take time. If your provider proxies this hostname, use DNS-only mode
            while verifying the origin record.
          </p>
          <p
            v-else-if="domain.dns_status === 'unavailable'"
            class="text-[11px] leading-4 text-muted-foreground"
          >
            Ignitify could not reach its resolver. The record may still be correct; retry shortly.
          </p>
          <p
            v-if="formatCheckedAt(domain.dns_checked_at)"
            class="text-[11px] text-muted-foreground"
          >
            Last checked {{ formatCheckedAt(domain.dns_checked_at) }}
          </p>
          <span v-if="domain.last_error" class="text-xs text-destructive">{{
            domain.last_error
          }}</span>
        </div>

        <div class="flex flex-wrap items-center gap-2 lg:justify-end">
          <Button
            variant="ghost"
            class="inline-flex h-8 items-center gap-1.5 rounded-[3px] border border-border px-2.5 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-50"
            type="button"
            :disabled="domain.dns_status === 'pending'"
            :aria-label="`Verify DNS for ${domain.hostname}`"
            title="Verify DNS"
            @click="emit('verify', domain)"
          >
            <RefreshCw
              class="size-3.5"
              :class="domain.dns_status === 'pending' ? 'animate-spin' : ''"
              :stroke-width="1.5"
            />
            Verify DNS
          </Button>
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
          <Button
            variant="ghost"
            v-if="props.canManage"
            class="grid size-8 place-items-center rounded-[3px] border border-transparent text-muted-foreground transition-colors hover:border-destructive/40 hover:bg-destructive/10 hover:text-destructive"
            type="button"
            :aria-label="`Remove ${domain.hostname}`"
            title="Remove domain"
            @click="emit('remove', domain)"
          >
            <Trash2 class="size-4" :stroke-width="1.5" />
          </Button>
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
