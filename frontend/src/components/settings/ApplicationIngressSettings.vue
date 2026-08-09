<script setup lang="ts">
import { Globe2, LockKeyhole, ShieldCheck } from "@lucide/vue";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import type { CertificateProvider, CustomCertificateSummary } from "./types";

interface Props {
  applicationDomainSuffix: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  acmeEmail: string;
  dnsRecordType: "a" | "cname";
  dnsRecordTarget: string;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
  domainError?: string;
  emailError?: string;
  dnsError?: string;
  tlsError?: string;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (event: "update:applicationDomainSuffix", value: string): void;
  (event: "update:httpsEnabled", value: boolean): void;
  (event: "update:automaticallyProvisionSsl", value: boolean): void;
  (event: "update:acmeEmail", value: string): void;
  (event: "update:dnsRecordType", value: "a" | "cname"): void;
  (event: "update:dnsRecordTarget", value: string): void;
  (event: "update:certificateProvider", value: CertificateProvider): void;
  (event: "update:customCertificateId", value: string | null): void;
}>();

function updateProvider(value: string | number) {
  const provider = String(value);
  emit(
    "update:certificateProvider",
    provider === "lets-encrypt" || provider === "custom" ? provider : "none",
  );
}

function updateCustomCertificate(value: string | number) {
  emit("update:customCertificateId", String(value));
}
</script>

<template>
  <section class="app-surface" aria-labelledby="application-ingress-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <Globe2 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Application ingress</p>
        <h2 id="application-ingress-heading" class="mt-1.5 text-base font-medium">
          Domain, DNS, and TLS policy
        </h2>
      </div>
    </header>

    <div class="grid gap-5 px-5 py-5">
      <div class="grid gap-2">
        <label for="application-domain-suffix" class="text-xs font-medium"
          >Managed domain suffix</label
        >
        <Input
          id="application-domain-suffix"
          :model-value="props.applicationDomainSuffix"
          class="rounded-[3px] font-mono text-sm"
          placeholder="apps.example.com"
          autocomplete="off"
          spellcheck="false"
          :aria-invalid="Boolean(props.domainError)"
          aria-describedby="application-domain-suffix-help application-domain-suffix-error"
          @update:model-value="emit('update:applicationDomainSuffix', String($event))"
        />
        <p id="application-domain-suffix-help" class="text-[11px] leading-4 text-muted-foreground">
          Used for generated platform hostnames, for example
          <code class="font-mono text-foreground">api.apps.example.com</code>. Custom domains may
          use any hostname allowed by the operator policy.
        </p>
        <p
          v-if="props.domainError"
          id="application-domain-suffix-error"
          class="text-[11px] text-destructive"
        >
          {{ props.domainError }}
        </p>
      </div>

      <div class="grid gap-4 border-t border-border pt-5">
        <div class="grid gap-2">
          <label for="dns-record-type" class="text-xs font-medium">DNS record type</label>
          <Select
            :model-value="props.dnsRecordType"
            @update:model-value="
              emit('update:dnsRecordType', String($event) === 'cname' ? 'cname' : 'a')
            "
          >
            <SelectTrigger id="dns-record-type" class="w-full rounded-[3px]">
              <SelectValue placeholder="Select DNS record type" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="a">A record</SelectItem>
              <SelectItem value="cname">CNAME record</SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="grid gap-2">
          <label for="dns-record-target" class="text-xs font-medium">DNS record target</label>
          <Input
            id="dns-record-target"
            :model-value="props.dnsRecordTarget"
            class="rounded-[3px] font-mono text-sm"
            :placeholder="props.dnsRecordType === 'a' ? '203.0.113.10' : 'edge.example.com'"
            autocomplete="off"
            spellcheck="false"
            :aria-invalid="Boolean(props.dnsError)"
            aria-describedby="dns-record-target-help dns-record-target-error"
            @update:model-value="emit('update:dnsRecordTarget', String($event))"
          />
          <p id="dns-record-target-help" class="text-[11px] leading-4 text-muted-foreground">
            The target shown to users when they configure a custom domain at their DNS provider.
          </p>
          <p
            v-if="props.dnsError"
            id="dns-record-target-error"
            class="text-[11px] text-destructive"
          >
            {{ props.dnsError }}
          </p>
        </div>
      </div>

      <div class="flex items-start justify-between gap-4 border-t border-border pt-5">
        <div class="flex min-w-0 items-start gap-3">
          <LockKeyhole class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
          <div>
            <label for="https-enabled" class="text-xs font-medium">Default HTTPS</label>
            <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
              Use TLS for managed application routes and redirect their HTTP traffic.
            </p>
          </div>
        </div>
        <Switch
          id="https-enabled"
          class="mt-0.5"
          :model-value="props.httpsEnabled"
          aria-label="Default HTTPS"
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
              <label for="automatic-ssl" class="text-xs font-medium">Automatic certificates</label>
              <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
                Request and renew certificates with Let's Encrypt.
              </p>
            </div>
          </div>
          <Switch
            id="automatic-ssl"
            class="mt-0.5"
            :disabled="!props.httpsEnabled || props.certificateProvider !== 'lets-encrypt'"
            :model-value="props.automaticallyProvisionSsl"
            aria-label="Automatic certificates"
            @update:model-value="emit('update:automaticallyProvisionSsl', $event)"
          />
        </div>

        <div class="grid gap-2">
          <label for="certificate-provider" class="text-xs font-medium">Certificate source</label>
          <Select
            :disabled="!props.httpsEnabled"
            :model-value="props.certificateProvider"
            @update:model-value="updateProvider"
          >
            <SelectTrigger id="certificate-provider" class="w-full rounded-[3px]">
              <SelectValue placeholder="Select certificate source" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">None</SelectItem>
              <SelectItem value="lets-encrypt">Let's Encrypt</SelectItem>
              <SelectItem value="custom" :disabled="!props.customCertificates.length">
                Custom certificate
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <div class="grid gap-2">
          <label for="acme-email" class="text-xs font-medium">ACME contact email</label>
          <Input
            id="acme-email"
            type="email"
            :model-value="props.acmeEmail"
            class="rounded-[3px]"
            placeholder="ops@example.com"
            autocomplete="email"
            :disabled="!props.httpsEnabled || props.certificateProvider !== 'lets-encrypt'"
            :aria-invalid="Boolean(props.emailError)"
            aria-describedby="acme-email-help acme-email-error"
            @update:model-value="emit('update:acmeEmail', String($event))"
          />
          <p id="acme-email-help" class="text-[11px] leading-4 text-muted-foreground">
            Used for certificate expiry and renewal notices.
          </p>
          <p v-if="props.emailError" id="acme-email-error" class="text-[11px] text-destructive">
            {{ props.emailError }}
          </p>
        </div>

        <div v-if="props.certificateProvider === 'custom'" class="grid gap-2">
          <label for="custom-certificate" class="text-xs font-medium">Custom certificate</label>
          <Select
            :disabled="!props.httpsEnabled"
            :model-value="props.customCertificateId ?? undefined"
            @update:model-value="updateCustomCertificate"
          >
            <SelectTrigger id="custom-certificate" class="w-full rounded-[3px]">
              <SelectValue placeholder="Select a custom certificate" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="certificate in props.customCertificates"
                :key="certificate.id"
                :value="certificate.id"
              >
                {{ certificate.name }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <p v-if="props.tlsError" class="text-[11px] text-destructive" role="alert">
          {{ props.tlsError }}
        </p>
      </div>
    </div>
  </section>
</template>
