<script setup lang="ts">
import {
  CircleCheck,
  CircleX,
  Globe2,
  LockKeyhole,
  RefreshCw,
  Server,
  ShieldCheck,
} from "@lucide/vue";
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { ServiceSummary } from "@/lib/types";

export type CertificateProvider = "none" | "lets-encrypt";
export type DnsValidationState = "idle" | "checking" | "valid" | "missing" | "unavailable";

const props = defineProps<{
  services: ServiceSummary[];
  serverDomain: string;
  serviceId: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  certificateProvider: CertificateProvider;
  domainError: string;
  tlsError: string;
  dnsState: DnsValidationState;
  dnsMessage: string;
}>();

const emit = defineEmits<{
  "update:serverDomain": [value: string];
  "update:serviceId": [value: string];
  "update:httpsEnabled": [value: boolean];
  "update:automaticallyProvisionSsl": [value: boolean];
  "update:certificateProvider": [value: CertificateProvider];
  create: [];
  validateDns: [];
}>();

const routableServices = computed(() => props.services.filter((service) => service.internal_port));
const canSubmit = computed(() => Boolean(props.serviceId) && !props.domainError && !props.tlsError);

function updateProvider(value: string | number | undefined) {
  const provider = String(value ?? "");
  emit("update:certificateProvider", provider === "lets-encrypt" ? provider : "none");
}

function dnsStatusLabel(state: DnsValidationState) {
  if (state === "checking") return "Checking DNS records...";
  if (state === "valid") return "DNS record found";
  if (state === "missing") return "DNS record not found";
  if (state === "unavailable") return "DNS check unavailable";
  return "";
}
</script>

<template>
  <section class="app-surface" aria-labelledby="domain-configuration-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <Globe2 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Ingress</p>
        <h2 id="domain-configuration-heading" class="mt-1.5 text-base font-medium">
          Domain configuration
        </h2>
        <p class="mt-1.5 max-w-[60ch] text-xs leading-5 text-muted-foreground">
          Point a public hostname at one service and choose how HTTPS certificates are managed.
        </p>
      </div>
    </header>

    <form class="grid gap-5 px-5 py-5" @submit.prevent="emit('create')">
      <div class="grid gap-2">
        <div class="flex items-center justify-between gap-3">
          <label for="project-domain" class="text-xs font-medium">Server domain</label>
          <span class="font-mono text-[10px] text-muted-foreground">FQDN</span>
        </div>
        <div class="flex items-start gap-2 max-[520px]:flex-col">
          <Input
            id="project-domain"
            class="min-w-0 flex-1 rounded-[3px] font-mono text-sm"
            :model-value="props.serverDomain"
            placeholder="app.example.com"
            autocomplete="off"
            spellcheck="false"
            :aria-invalid="Boolean(props.domainError)"
            aria-describedby="project-domain-help project-domain-error"
            @update:model-value="emit('update:serverDomain', String($event))"
          />
          <Button
            class="shrink-0 max-[520px]:w-full"
            size="sm"
            type="button"
            variant="outline"
            :disabled="Boolean(props.domainError) || props.dnsState === 'checking'"
            @click="emit('validateDns')"
          >
            <RefreshCw
              class="size-4"
              :class="props.dnsState === 'checking' ? 'animate-spin' : ''"
              :stroke-width="1.5"
            />
            Validate DNS
          </Button>
        </div>
        <p id="project-domain-help" class="text-[11px] leading-4 text-muted-foreground">
          Hostname only, without <code class="font-mono text-foreground">\`http://\`</code> or a
          path.
        </p>
        <p v-if="props.domainError" id="project-domain-error" class="text-[11px] text-destructive">
          {{ props.domainError }}
        </p>
        <p
          v-else-if="props.dnsState !== 'idle'"
          class="flex items-center gap-1.5 text-[11px]"
          :class="
            props.dnsState === 'valid'
              ? 'text-metric-green'
              : props.dnsState === 'checking'
                ? 'text-muted-foreground'
                : 'text-destructive'
          "
          role="status"
          aria-live="polite"
        >
          <CircleCheck v-if="props.dnsState === 'valid'" class="size-3.5" :stroke-width="1.7" />
          <CircleX v-else-if="props.dnsState !== 'checking'" class="size-3.5" :stroke-width="1.7" />
          <RefreshCw v-else class="size-3.5 animate-spin" :stroke-width="1.7" />
          {{ props.dnsMessage || dnsStatusLabel(props.dnsState) }}
        </p>
      </div>

      <div class="grid gap-2 border-t border-border pt-5">
        <label for="domain-service" class="flex items-center gap-2 text-xs font-medium">
          <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
          Service
        </label>
        <Select
          :model-value="props.serviceId || undefined"
          :disabled="!routableServices.length"
          @update:model-value="(value) => emit('update:serviceId', String(value ?? ''))"
        >
          <SelectTrigger id="domain-service" class="w-full rounded-[3px]">
            <SelectValue placeholder="Select service that handles the port" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="service in routableServices" :key="service.id" :value="service.id">
              <span class="flex items-center gap-2">
                <span>{{ service.name }}</span>
                <span class="font-mono text-[10px] text-muted-foreground"
                  >:{{ service.internal_port }}</span
                >
              </span>
            </SelectItem>
          </SelectContent>
        </Select>
        <p class="text-[11px] leading-4 text-muted-foreground">
          Only services with an internal port can receive public traffic.
        </p>
        <p v-if="!routableServices.length" class="text-[11px] text-destructive">
          Configure an internal port on a service before adding a domain.
        </p>
      </div>

      <div class="flex items-start justify-between gap-4 border-t border-border pt-5">
        <div class="flex min-w-0 items-start gap-3">
          <LockKeyhole class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
          <div>
            <label for="domain-https" class="text-xs font-medium">Enable HTTPS</label>
            <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
              Serve this domain over TLS and use an encrypted public connection.
            </p>
          </div>
        </div>
        <Switch
          id="domain-https"
          class="mt-0.5"
          :model-value="props.httpsEnabled"
          aria-label="Enable HTTPS"
          @update:model-value="emit('update:httpsEnabled', $event)"
        />
      </div>

      <div
        class="grid gap-5 border-t border-border pt-5"
        :class="!props.httpsEnabled ? 'opacity-55' : ''"
      >
        <div class="flex items-start justify-between gap-4">
          <div class="flex min-w-0 items-start gap-3">
            <ShieldCheck class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
            <div>
              <label for="domain-automatic-ssl" class="text-xs font-medium"
                >Automatically provision SSL</label
              >
              <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
                Request and renew certificates automatically through the selected provider.
              </p>
            </div>
          </div>
          <Switch
            id="domain-automatic-ssl"
            class="mt-0.5"
            :disabled="!props.httpsEnabled || props.certificateProvider !== 'lets-encrypt'"
            :model-value="props.automaticallyProvisionSsl"
            aria-label="Automatically provision SSL"
            @update:model-value="emit('update:automaticallyProvisionSsl', $event)"
          />
        </div>

        <div class="grid gap-2">
          <label for="domain-certificate-provider" class="text-xs font-medium"
            >Certificate provider</label
          >
          <Select
            :disabled="!props.httpsEnabled"
            :model-value="props.certificateProvider"
            @update:model-value="updateProvider"
          >
            <SelectTrigger id="domain-certificate-provider" class="w-full rounded-[3px]">
              <SelectValue placeholder="Select certificate provider" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">None</SelectItem>
              <SelectItem value="lets-encrypt">Let's Encrypt</SelectItem>
            </SelectContent>
          </Select>
          <p class="text-[11px] leading-4 text-muted-foreground">
            Select <span class="font-medium text-foreground">Let's Encrypt</span> to enable
            automatic SSL provisioning.
          </p>
        </div>

        <p v-if="props.tlsError" class="text-[11px] text-destructive" role="alert">
          {{ props.tlsError }}
        </p>
      </div>

      <div class="flex items-center justify-between gap-4 border-t border-border pt-4">
        <p class="text-[11px] leading-4 text-muted-foreground">
          The route is created in a pending state and reconciled by the control plane.
        </p>
        <Button class="shrink-0" size="sm" type="submit" :disabled="!canSubmit">
          Add domain
        </Button>
      </div>
    </form>
  </section>
</template>
