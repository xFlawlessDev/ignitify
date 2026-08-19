<script setup lang="ts">
import { Globe2, LockKeyhole, ShieldCheck } from "@lucide/vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
  publicOrigin: string;
  controlPlaneDomain: string;
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
const { t } = useI18n();

function configuredPublicOrigin(value: string): URL | null {
  try {
    const origin = new URL(value.trim());
    const isLoopback =
      origin.hostname === "localhost" ||
      origin.hostname === "127.0.0.1" ||
      origin.hostname === "::1";
    return origin.protocol === "https:" && !isLoopback ? origin : null;
  } catch {
    return null;
  }
}

const serviceWildcard = computed(() => {
  const suffix = props.applicationDomainSuffix.trim().toLowerCase();
  return `*.${suffix || "apps.example.com"}`;
});
const dnsTarget = computed(() => props.dnsRecordTarget.trim() || "<public-vps-ip>");
const currentPublicOrigin = computed(() => configuredPublicOrigin(props.publicOrigin));
const controlPlaneHostname = computed(
  () =>
    props.controlPlaneDomain.trim().toLowerCase() ||
    currentPublicOrigin.value?.hostname ||
    "console.example.com",
);
const controlPlaneOrigin = computed(() => `https://${controlPlaneHostname.value}`);

const isCloudflareTunnel = computed(
  () =>
    props.dnsRecordType === "cname" &&
    props.dnsRecordTarget.toLowerCase().endsWith(".cfargotunnel.com"),
);
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
        <Label for="application-domain-suffix" class="text-xs font-medium"
          >Managed domain suffix</Label
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
          <Label for="dns-record-type" class="text-xs font-medium">DNS record type</Label>
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
          <Label for="dns-record-target" class="text-xs font-medium">DNS record target</Label>
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

      <section
        v-if="isCloudflareTunnel"
        class="grid gap-4 border-t border-border pt-5"
        aria-labelledby="cloudflare-tunnel-guide-heading"
      >
        <div>
          <p class="ui-label">Cloudflare Tunnel</p>
          <h3 id="cloudflare-tunnel-guide-heading" class="mt-1.5 text-sm font-medium">
            Published application setup
          </h3>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            Use the configured hostnames and replace the tunnel ID with a value from your own
            environment.
          </p>
        </div>
        <ol class="grid gap-3 text-xs leading-5 text-muted-foreground">
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">01</span>
            <p>
              In Cloudflare DNS, create a proxied CNAME record:
              <code class="font-mono text-foreground">{{ serviceWildcard }}</code>
              to
              <code class="break-all font-mono text-foreground"
                >&lt;tunnel-id&gt;.cfargotunnel.com</code
              >.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">02</span>
            <p>
              In the Tunnel, add a published application route from
              <code class="font-mono text-foreground">{{ serviceWildcard }}</code>
              to
              <code class="font-mono text-foreground">http://127.0.0.1:80</code>.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">03</span>
            <p>
              {{ t("ingressSetup.cloudflareControlPlaneDnsPrefix") }}
              <code class="break-all font-mono text-foreground">{{ controlPlaneHostname }}</code>
              to
              <code class="break-all font-mono text-foreground"
                >&lt;tunnel-id&gt;.cfargotunnel.com</code
              >.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">04</span>
            <p>
              {{ t("ingressSetup.cloudflareControlPlaneRoutePrefix") }}
              <code class="break-all font-mono text-foreground">{{ controlPlaneHostname }}</code>
              to
              <code class="font-mono text-foreground">http://127.0.0.1:5656</code>. This is an
              {{ t("ingressSetup.cloudflareExternalProxySuffix") }}
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">05</span>
            <p>
              {{ t("ingressSetup.cloudflareExternalConfiguration") }}
              <code class="font-mono text-foreground">IGNITIFY_REMOTE_MODE=true</code> and an HTTPS
              <code class="break-all font-mono text-foreground"
                >IGNITIFY_TRUSTED_ORIGINS={{ controlPlaneOrigin }}</code
              >.
              {{ t("ingressSetup.cloudflareProxyHeaders") }}
              <code class="font-mono text-foreground">IGNITIFY_SECURE_COOKIES=true</code>,
              {{ t("ingressSetup.cloudflareRestart") }}
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">06</span>
            <p>
              {{ t("ingressSetup.cloudflareTls") }}
            </p>
          </li>
        </ol>
      </section>

      <section
        v-else-if="props.dnsRecordType === 'a'"
        class="grid gap-4 border-t border-border pt-5"
        aria-labelledby="public-vps-guide-heading"
      >
        <div>
          <p class="ui-label">{{ t("ingressSetup.publicVps") }}</p>
          <h3 id="public-vps-guide-heading" class="mt-1.5 text-sm font-medium">
            {{ t("ingressSetup.title") }}
          </h3>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            {{ t("ingressSetup.description") }}
          </p>
        </div>
        <ol class="grid gap-3 text-xs leading-5 text-muted-foreground">
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">01</span>
            <p>
              {{ t("ingressSetup.applicationDnsPrefix") }}
              <code class="break-all font-mono text-foreground">{{ serviceWildcard }}</code>
              {{ t("ingressSetup.to") }}
              <code class="break-all font-mono text-foreground">{{ dnsTarget }}</code
              >.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">02</span>
            <p>
              {{ t("ingressSetup.controlPlaneDnsPrefix") }}
              <code class="break-all font-mono text-foreground">{{ controlPlaneHostname }}</code>
              {{ t("ingressSetup.to") }}
              <code class="break-all font-mono text-foreground">{{ dnsTarget }}</code
              >.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">03</span>
            <p>
              {{ t("ingressSetup.proxyPrefix") }}
              <code class="break-all font-mono text-foreground">{{ controlPlaneOrigin }}</code>
              {{ t("ingressSetup.proxySuffix") }}
              <code class="font-mono text-foreground">127.0.0.1:5656</code>.
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">04</span>
            <p>
              {{ t("ingressSetup.firewallPrefix") }}
              <code class="font-mono text-foreground">80</code>
              {{ t("ingressSetup.and") }}
              <code class="font-mono text-foreground">443</code>
              {{ t("ingressSetup.firewallSuffix") }}
              <code class="font-mono text-foreground">5656</code>
              {{ t("ingressSetup.private") }}
            </p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">05</span>
            <p>{{ t("ingressSetup.managedControlPlane") }}</p>
          </li>
          <li class="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-2">
            <span class="font-mono text-foreground">06</span>
            <p>{{ t("ingressSetup.tls") }}</p>
          </li>
        </ol>
      </section>

      <div class="flex items-start justify-between gap-4 border-t border-border pt-5">
        <div class="flex min-w-0 items-start gap-3">
          <LockKeyhole class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
          <div>
            <Label for="https-enabled" class="text-xs font-medium">Default HTTPS</Label>
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
              <Label for="automatic-ssl" class="text-xs font-medium">Automatic certificates</Label>
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
          <Label for="certificate-provider" class="text-xs font-medium">Certificate source</Label>
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
          <Label for="acme-email" class="text-xs font-medium">ACME contact email</Label>
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
          <Label for="custom-certificate" class="text-xs font-medium">Custom certificate</Label>
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
