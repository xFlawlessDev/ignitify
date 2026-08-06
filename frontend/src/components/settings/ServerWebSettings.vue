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
  serverDomain: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
  domainError?: string;
  tlsError?: string;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (event: "update:serverDomain", value: string): void;
  (event: "update:httpsEnabled", value: boolean): void;
  (event: "update:automaticallyProvisionSsl", value: boolean): void;
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
  <section class="border border-border bg-card" aria-labelledby="web-server-heading">
    <header class="flex items-start gap-3 border-b border-border px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center border border-border bg-muted text-muted-foreground"
      >
        <Globe2 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Web server</p>
        <h2 id="web-server-heading" class="mt-1.5 text-base font-medium">Public entrypoint</h2>
        <p class="mt-1.5 max-w-[52ch] text-xs leading-5 text-muted-foreground">
          Set the hostname and TLS policy used for the control plane and managed ingress.
        </p>
      </div>
    </header>

    <div class="grid gap-5 px-5 py-5">
      <div class="grid gap-2">
        <label for="server-domain" class="text-xs font-medium">Server domain</label>
        <Input
          id="server-domain"
          :model-value="props.serverDomain"
          class="rounded-[3px] font-mono text-sm"
          placeholder="deploy.example.com"
          autocomplete="off"
          spellcheck="false"
          :aria-invalid="Boolean(props.domainError)"
          aria-describedby="server-domain-help server-domain-error"
          @update:model-value="emit('update:serverDomain', String($event))"
        />
        <p id="server-domain-help" class="text-[11px] leading-4 text-muted-foreground">
          Hostname only, without <code class="font-mono text-foreground">http://</code> or a path.
        </p>
        <p v-if="props.domainError" id="server-domain-error" class="text-[11px] text-destructive">
          {{ props.domainError }}
        </p>
      </div>

      <div class="flex items-start justify-between gap-4 border-t border-border pt-5">
        <div class="flex min-w-0 items-start gap-3">
          <LockKeyhole class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
          <div>
            <label for="https-enabled" class="text-xs font-medium">Enable HTTPS</label>
            <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
              Serve the server domain over TLS and configure its certificate source.
            </p>
          </div>
        </div>
        <Switch
          id="https-enabled"
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
              <label for="automatic-ssl" class="text-xs font-medium"
                >Automatically provision SSL</label
              >
              <p class="mt-1 max-w-[48ch] text-[11px] leading-4 text-muted-foreground">
                Request and renew certificates automatically through the selected provider.
              </p>
            </div>
          </div>
          <Switch
            id="automatic-ssl"
            class="mt-0.5"
            :disabled="!props.httpsEnabled || props.certificateProvider !== 'lets-encrypt'"
            :model-value="props.automaticallyProvisionSsl"
            aria-label="Automatically provision SSL"
            @update:model-value="emit('update:automaticallyProvisionSsl', $event)"
          />
        </div>

        <div class="grid gap-2">
          <label for="certificate-provider" class="text-xs font-medium">Certificate provider</label>
          <Select
            :disabled="!props.httpsEnabled"
            :model-value="props.certificateProvider"
            @update:model-value="updateProvider"
          >
            <SelectTrigger id="certificate-provider" class="w-full rounded-[3px]">
              <SelectValue placeholder="Select certificate provider" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="none">None</SelectItem>
              <SelectItem value="lets-encrypt">Let's Encrypt</SelectItem>
              <SelectItem value="custom" :disabled="!props.customCertificates.length">
                Custom certificate
              </SelectItem>
            </SelectContent>
          </Select>
          <p class="text-[11px] leading-4 text-muted-foreground">
            Select <span class="font-medium text-foreground">Let's Encrypt</span> to enable
            automatic SSL provisioning.
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
          <p class="text-[11px] leading-4 text-muted-foreground">
            Add certificates in the Custom certificates section before selecting one here.
          </p>
        </div>

        <p v-if="props.tlsError" class="text-[11px] text-destructive" role="alert">
          {{ props.tlsError }}
        </p>
      </div>
    </div>
  </section>
</template>
