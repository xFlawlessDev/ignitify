<script setup lang="ts">
import { Check, RefreshCw, RotateCcw, Save, Settings2 } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import ApplicationEnvironment from "@/components/settings/ApplicationEnvironment.vue";
import ApplicationIngressSettings from "@/components/settings/ApplicationIngressSettings.vue";
import CertificateManager from "@/components/settings/CertificateManager.vue";
import InfrastructureHealth from "@/components/settings/InfrastructureHealth.vue";
import type {
  CertificateProvider,
  CustomCertificateSummary,
  CustomCertificateUpload,
} from "@/components/settings/types";
import { Button } from "@/components/ui/button";
import {
  apiCreateInfrastructureCertificate,
  apiDeleteInfrastructureCertificate,
  apiGetInfrastructureSettings,
  apiUpdateInfrastructureSettings,
} from "@/lib/api";
import type {
  ApplicationEnvironmentStatus,
  InfrastructureHealthStatus,
  InfrastructureSettingsResponse,
} from "@/lib/api/settings";

interface SettingsDraft {
  applicationDomainSuffix: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  acmeEmail: string;
  dnsRecordType: "a" | "cname";
  dnsRecordTarget: string;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
}

const defaults: SettingsDraft = {
  applicationDomainSuffix: "",
  httpsEnabled: false,
  automaticallyProvisionSsl: false,
  acmeEmail: "",
  dnsRecordType: "a",
  dnsRecordTarget: "",
  certificateProvider: "none",
  customCertificateId: null,
  customCertificates: [],
};

function cloneSettings(settings: SettingsDraft): SettingsDraft {
  return {
    ...settings,
    customCertificates: settings.customCertificates.map((certificate) => ({ ...certificate })),
  };
}

function toDraft(settings: InfrastructureSettingsResponse): SettingsDraft {
  const customCertificates: CustomCertificateSummary[] = settings.certificates.map(
    (certificate) => ({
      id: certificate.id,
      name: certificate.name,
      certificateFileName: certificate.certificate_file_name,
      privateKeyFileName: certificate.private_key_file_name,
    }),
  );
  const certificateProvider: CertificateProvider =
    settings.certificate_provider === "lets-encrypt" || settings.certificate_provider === "custom"
      ? settings.certificate_provider
      : "none";
  const customCertificateId =
    certificateProvider === "custom" &&
    settings.custom_certificate_id &&
    customCertificates.some((certificate) => certificate.id === settings.custom_certificate_id)
      ? settings.custom_certificate_id
      : null;

  return {
    applicationDomainSuffix: settings.application_domain_suffix,
    httpsEnabled: settings.https_enabled,
    automaticallyProvisionSsl:
      settings.https_enabled &&
      settings.automatically_provision_ssl &&
      certificateProvider === "lets-encrypt",
    acmeEmail: settings.acme_email,
    dnsRecordType: settings.dns_record_type,
    dnsRecordTarget: settings.dns_record_target,
    certificateProvider:
      customCertificateId || certificateProvider !== "custom" ? certificateProvider : "none",
    customCertificateId,
    customCertificates,
  };
}

const draft = reactive<SettingsDraft>(cloneSettings(defaults));
const savedSettings = shallowRef<SettingsDraft | null>(null);
const applicationEnvironment = shallowRef<ApplicationEnvironmentStatus | null>(null);
const infrastructureHealth = shallowRef<InfrastructureHealthStatus | null>(null);
const saveState = shallowRef<"loading" | "idle" | "saving" | "saved" | "error">("loading");
const requestError = shallowRef("");

const domainError = computed(() => {
  const value = draft.applicationDomainSuffix.trim();
  if (!value) return "Application domain suffix is required.";
  if (
    value.length > 253 ||
    value.includes("..") ||
    !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(value)
  ) {
    return "Use a valid hostname without a protocol or path.";
  }
  return "";
});

const tlsError = computed(() => {
  if (!draft.httpsEnabled) return "";
  if (draft.automaticallyProvisionSsl && draft.certificateProvider !== "lets-encrypt") {
    return "Automatic certificates require Let's Encrypt.";
  }
  if (draft.certificateProvider === "custom") {
    if (!draft.customCertificateId) return "Select a custom certificate.";
    if (
      !draft.customCertificates.some((certificate) => certificate.id === draft.customCertificateId)
    ) {
      return "The selected custom certificate is no longer available.";
    }
  }
  return "";
});

const emailError = computed(() => {
  if (
    !draft.httpsEnabled ||
    !draft.automaticallyProvisionSsl ||
    draft.certificateProvider !== "lets-encrypt"
  ) {
    return "";
  }
  if (!draft.acmeEmail.trim()) return "ACME contact email is required for automatic certificates.";
  if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(draft.acmeEmail.trim())) {
    return "Use a valid email address.";
  }
  return "";
});

const dnsError = computed(() => {
  const value = draft.dnsRecordTarget.trim();
  if (!value) return "";
  if (draft.dnsRecordType === "a") {
    const octets = value.split(".");
    if (
      octets.length !== 4 ||
      octets.some((octet) => !/^\d+$/.test(octet) || Number(octet) > 255)
    ) {
      return "Use a valid IPv4 address for an A record.";
    }
    return "";
  }
  if (
    value.length > 253 ||
    value.includes("..") ||
    !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(value)
  ) {
    return "Use a valid hostname for a CNAME record.";
  }
  return "";
});

const isDirty = computed(
  () =>
    savedSettings.value !== null && JSON.stringify(draft) !== JSON.stringify(savedSettings.value),
);
const canSave = computed(
  () =>
    saveState.value !== "loading" &&
    saveState.value !== "saving" &&
    isDirty.value &&
    !domainError.value &&
    !tlsError.value &&
    !emailError.value &&
    !dnsError.value,
);

function markDirty() {
  if (saveState.value !== "loading" && saveState.value !== "saving") {
    saveState.value = "idle";
  }
  requestError.value = "";
}

function applySettings(settings: InfrastructureSettingsResponse) {
  const next = toDraft(settings);
  Object.assign(draft, next);
  savedSettings.value = cloneSettings(next);
  applicationEnvironment.value = settings.application;
  infrastructureHealth.value = settings.health;
}

function updateHttpsEnabled(value: boolean) {
  draft.httpsEnabled = value;
  if (!value) {
    draft.automaticallyProvisionSsl = false;
    draft.certificateProvider = "none";
    draft.customCertificateId = null;
  }
  markDirty();
}

function updateCertificateProvider(value: CertificateProvider) {
  draft.certificateProvider = value;
  if (value !== "lets-encrypt") draft.automaticallyProvisionSsl = false;
  if (value !== "custom") draft.customCertificateId = null;
  markDirty();
}

async function addCertificate(upload: CustomCertificateUpload) {
  requestError.value = "";
  const result = await apiCreateInfrastructureCertificate(
    upload.name,
    upload.certificateFile,
    upload.privateKeyFile,
  );
  if (!result.success) {
    requestError.value = result.error ?? "Unable to upload certificate.";
    saveState.value = "error";
    return;
  }
  const certificate: CustomCertificateSummary = {
    id: result.data.id,
    name: result.data.name,
    certificateFileName: result.data.certificate_file_name,
    privateKeyFileName: result.data.private_key_file_name,
  };
  draft.customCertificates.push(certificate);
  if (savedSettings.value) {
    savedSettings.value = cloneSettings({
      ...savedSettings.value,
      customCertificates: [...savedSettings.value.customCertificates, certificate],
    });
  }
  saveState.value = "idle";
}

async function removeCertificate(certificateId: string) {
  requestError.value = "";
  const result = await apiDeleteInfrastructureCertificate(certificateId);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove certificate.";
    saveState.value = "error";
    return;
  }
  draft.customCertificates = draft.customCertificates.filter(
    (certificate) => certificate.id !== certificateId,
  );
  if (draft.customCertificateId === certificateId) {
    draft.customCertificateId = null;
    draft.certificateProvider = "none";
    draft.automaticallyProvisionSsl = false;
  }
  if (savedSettings.value) {
    savedSettings.value = cloneSettings({
      ...savedSettings.value,
      customCertificates: savedSettings.value.customCertificates.filter(
        (certificate) => certificate.id !== certificateId,
      ),
      ...(savedSettings.value.customCertificateId === certificateId
        ? {
            customCertificateId: null,
            certificateProvider: "none" as const,
            automaticallyProvisionSsl: false,
          }
        : {}),
    });
  }
  saveState.value = "idle";
}

async function saveSettings() {
  if (!canSave.value) return;

  saveState.value = "saving";
  requestError.value = "";
  const result = await apiUpdateInfrastructureSettings({
    application_domain_suffix: draft.applicationDomainSuffix.trim(),
    https_enabled: draft.httpsEnabled,
    automatically_provision_ssl: draft.automaticallyProvisionSsl,
    acme_email: draft.acmeEmail.trim(),
    dns_record_type: draft.dnsRecordType,
    dns_record_target: draft.dnsRecordTarget.trim(),
    certificate_provider: draft.certificateProvider,
    custom_certificate_id: draft.customCertificateId,
  });
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save infrastructure settings.";
    saveState.value = "error";
    return;
  }
  applySettings(result.data);
  saveState.value = "saved";
}

function resetSettings() {
  if (!savedSettings.value) return;
  Object.assign(draft, cloneSettings(savedSettings.value));
  requestError.value = "";
  saveState.value = "idle";
}

async function loadSettings() {
  if (saveState.value === "saving") return;
  saveState.value = "loading";
  requestError.value = "";
  const result = await apiGetInfrastructureSettings();
  if (!result.success) {
    requestError.value = result.error ?? "Unable to load infrastructure settings.";
    saveState.value = "error";
    return;
  }
  applySettings(result.data);
  saveState.value = "idle";
}

onMounted(loadSettings);
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Infrastructure</h1>
        <p class="mt-2 max-w-[58ch] text-sm leading-5 text-muted-foreground">
          Configure managed application routing, certificates, and runtime health checks.
        </p>
      </div>
      <div class="flex w-full items-center justify-between gap-3 sm:w-auto sm:justify-end">
        <span
          class="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground"
          role="status"
          aria-live="polite"
        >
          <Check
            v-if="saveState === 'saved' && !isDirty"
            class="size-3.5 text-metric-green"
            :stroke-width="1.7"
          />
          <span v-if="saveState === 'saved' && !isDirty">Saved to server</span>
          <span v-else-if="saveState === 'loading'">Loading infrastructure</span>
          <span v-else-if="saveState === 'saving'">Saving infrastructure</span>
          <span v-else-if="saveState === 'error'">Infrastructure unavailable</span>
          <span v-else-if="isDirty">Unsaved changes</span>
          <span v-else>No unsaved changes</span>
        </span>
        <Button
          v-if="!isDirty"
          variant="outline"
          size="sm"
          type="button"
          :disabled="saveState === 'loading' || saveState === 'saving'"
          @click="loadSettings"
        >
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Refresh
        </Button>
        <Button v-if="isDirty" variant="ghost" size="sm" type="button" @click="resetSettings">
          <RotateCcw class="size-4" :stroke-width="1.5" />
          Reset
        </Button>
        <Button size="sm" type="button" :disabled="!canSave" @click="saveSettings">
          <Save class="size-4" :stroke-width="1.5" />
          Save changes
        </Button>
      </div>
    </header>

    <p v-if="requestError" class="mt-4 text-[11px] text-destructive" role="alert">
      {{ requestError }}
    </p>

    <div class="mt-6 grid gap-4">
      <ApplicationEnvironment :environment="applicationEnvironment" />
      <InfrastructureHealth :health="infrastructureHealth" />

      <form class="grid gap-4" @submit.prevent="saveSettings">
        <div class="flex items-center gap-2 border-b border-border pb-3">
          <Settings2 class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <p class="ui-label">Managed application ingress</p>
        </div>

        <ApplicationIngressSettings
          :application-domain-suffix="draft.applicationDomainSuffix"
          :https-enabled="draft.httpsEnabled"
          :automatically-provision-ssl="draft.automaticallyProvisionSsl"
          :acme-email="draft.acmeEmail"
          :dns-record-type="draft.dnsRecordType"
          :dns-record-target="draft.dnsRecordTarget"
          :certificate-provider="draft.certificateProvider"
          :custom-certificate-id="draft.customCertificateId"
          :custom-certificates="draft.customCertificates"
          :domain-error="isDirty ? domainError : ''"
          :email-error="isDirty ? emailError : ''"
          :dns-error="isDirty ? dnsError : ''"
          :tls-error="isDirty ? tlsError : ''"
          @update:application-domain-suffix="
            draft.applicationDomainSuffix = $event;
            markDirty();
          "
          @update:https-enabled="updateHttpsEnabled"
          @update:acme-email="
            draft.acmeEmail = $event;
            markDirty();
          "
          @update:dns-record-type="
            draft.dnsRecordType = $event;
            markDirty();
          "
          @update:dns-record-target="
            draft.dnsRecordTarget = $event;
            markDirty();
          "
          @update:automatically-provision-ssl="
            draft.automaticallyProvisionSsl = $event;
            markDirty();
          "
          @update:certificate-provider="updateCertificateProvider"
          @update:custom-certificate-id="
            draft.customCertificateId = $event;
            markDirty();
          "
        />

        <CertificateManager
          :certificates="draft.customCertificates"
          @add="addCertificate"
          @remove="removeCertificate"
        />

        <footer
          class="flex items-center justify-between gap-4 border-t border-border pt-4 text-[11px] text-muted-foreground max-[560px]:items-start max-[560px]:flex-col"
        >
          <p>Certificate and private key material are encrypted at rest.</p>
          <span class="shrink-0 font-mono">Admin only</span>
        </footer>
      </form>
    </div>
  </div>
</template>
