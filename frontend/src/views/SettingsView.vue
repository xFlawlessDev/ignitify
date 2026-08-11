<script setup lang="ts">
import {
  HardDriveDownload,
  LayoutDashboard,
  Network,
  RefreshCw,
  RotateCcw,
  Save,
} from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import ApplicationEnvironment from "@/components/settings/ApplicationEnvironment.vue";
import ApplicationIngressSettings from "@/components/settings/ApplicationIngressSettings.vue";
import BackupDestinationSettings from "@/components/settings/BackupDestinationSettings.vue";
import BuildCapacitySettings from "@/components/settings/BuildCapacitySettings.vue";
import CertificateManager from "@/components/settings/CertificateManager.vue";
import ControlPlaneIngressSettings from "@/components/settings/ControlPlaneIngressSettings.vue";
import IngressFallbackSettings from "@/components/settings/IngressFallbackSettings.vue";
import InfrastructureHealth from "@/components/settings/InfrastructureHealth.vue";
import type {
  CertificateProvider,
  CustomCertificateSummary,
  CustomCertificateUpload,
} from "@/components/settings/types";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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
  controlPlaneDomain: string;
  applicationDomainSuffix: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  acmeEmail: string;
  dnsRecordType: "a" | "cname";
  dnsRecordTarget: string;
  fallbackPageHeading: string;
  fallbackPageMessage: string;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
  concurrentBuilds: number;
}

type SettingsSection = "overview" | "ingress" | "backup";

const defaults: SettingsDraft = {
  controlPlaneDomain: "",
  applicationDomainSuffix: "",
  httpsEnabled: false,
  automaticallyProvisionSsl: false,
  acmeEmail: "",
  dnsRecordType: "a",
  dnsRecordTarget: "",
  fallbackPageHeading: "Application not found",
  fallbackPageMessage: "The requested hostname is not connected to an active application.",
  certificateProvider: "none",
  customCertificateId: null,
  customCertificates: [],
  concurrentBuilds: 2,
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
    controlPlaneDomain: settings.control_plane_domain,
    applicationDomainSuffix: settings.application_domain_suffix,
    httpsEnabled: settings.https_enabled,
    automaticallyProvisionSsl:
      settings.https_enabled &&
      settings.automatically_provision_ssl &&
      certificateProvider === "lets-encrypt",
    acmeEmail: settings.acme_email,
    dnsRecordType: settings.dns_record_type,
    dnsRecordTarget: settings.dns_record_target,
    fallbackPageHeading: settings.fallback_page_heading,
    fallbackPageMessage: settings.fallback_page_message,
    certificateProvider:
      customCertificateId || certificateProvider !== "custom" ? certificateProvider : "none",
    customCertificateId,
    customCertificates,
    concurrentBuilds: settings.concurrent_builds,
  };
}

const draft = reactive<SettingsDraft>(cloneSettings(defaults));
const activeSection = shallowRef<SettingsSection>("overview");
const savedSettings = shallowRef<SettingsDraft | null>(null);
const applicationEnvironment = shallowRef<ApplicationEnvironmentStatus | null>(null);
const infrastructureHealth = shallowRef<InfrastructureHealthStatus | null>(null);
const saveState = shallowRef<"loading" | "idle" | "saving" | "saved" | "error">("loading");
const requestError = shallowRef("");
const { t } = useI18n();

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

const controlPlaneDomainError = computed(() => {
  const value = draft.controlPlaneDomain.trim();
  if (!value) return "";
  if (
    value.length > 253 ||
    value.includes("..") ||
    !/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/i.test(value)
  ) {
    return t("controlPlaneIngress.invalidDomain");
  }
  const applicationSuffix = draft.applicationDomainSuffix.trim().toLowerCase();
  if (
    applicationSuffix &&
    (value.toLowerCase() === applicationSuffix ||
      value.toLowerCase().endsWith(`.${applicationSuffix}`))
  ) {
    return t("controlPlaneIngress.overlapsApplicationDomain");
  }
  if (!draft.httpsEnabled) return t("controlPlaneIngress.requiresHttps");
  if (
    draft.certificateProvider !== "custom" &&
    !(draft.certificateProvider === "lets-encrypt" && draft.automaticallyProvisionSsl)
  ) {
    return t("controlPlaneIngress.requiresCertificate");
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

function hasDisallowedControlCharacter(value: string, allowNewlines = false) {
  return [...value].some((character) => {
    const codePoint = character.codePointAt(0);
    return (
      codePoint !== undefined &&
      (codePoint < 32 || codePoint === 127) &&
      !(allowNewlines && codePoint === 10)
    );
  });
}

const fallbackHeadingError = computed(() => {
  const value = draft.fallbackPageHeading.trim();
  if (!value) return "A fallback page heading is required.";
  if ([...value].length > 100 || hasDisallowedControlCharacter(value)) {
    return "Use 1-100 characters without line breaks.";
  }
  return "";
});

const fallbackMessageError = computed(() => {
  const value = draft.fallbackPageMessage.trim();
  if (!value) return "A fallback page message is required.";
  if ([...value].length > 280 || hasDisallowedControlCharacter(value, true)) {
    return "Use 1-280 characters.";
  }
  return "";
});

const buildCapacityError = computed(() => {
  if (
    !Number.isInteger(draft.concurrentBuilds) ||
    draft.concurrentBuilds < 1 ||
    draft.concurrentBuilds > 32
  ) {
    return "Use a whole number from 1 to 32.";
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
    !controlPlaneDomainError.value &&
    !tlsError.value &&
    !emailError.value &&
    !dnsError.value &&
    !fallbackHeadingError.value &&
    !fallbackMessageError.value &&
    !buildCapacityError.value,
);

const sectionDescription = computed(() => {
  switch (activeSection.value) {
    case "ingress":
      return "Configure managed routing, TLS policy, and the unmatched-hostname page.";
    case "backup":
      return "Configure durable control-plane backup storage and recovery access.";
    default:
      return "Review host readiness, runtime defaults, and build capacity.";
  }
});

function selectSection(value: string) {
  if (value === "overview" || value === "ingress" || value === "backup") {
    activeSection.value = value;
  }
}

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
    toast.error("Certificate upload failed", { description: requestError.value });
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
  toast.success("Certificate uploaded", { description: `${certificate.name} is ready to use.` });
}

async function removeCertificate(certificateId: string) {
  requestError.value = "";
  const result = await apiDeleteInfrastructureCertificate(certificateId);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove certificate.";
    saveState.value = "error";
    toast.error("Could not remove certificate", { description: requestError.value });
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
  toast.success("Certificate removed");
}

async function saveSettings() {
  if (!canSave.value) return;

  saveState.value = "saving";
  requestError.value = "";
  const result = await apiUpdateInfrastructureSettings({
    control_plane_domain: draft.controlPlaneDomain.trim(),
    application_domain_suffix: draft.applicationDomainSuffix.trim(),
    https_enabled: draft.httpsEnabled,
    automatically_provision_ssl: draft.automaticallyProvisionSsl,
    acme_email: draft.acmeEmail.trim(),
    dns_record_type: draft.dnsRecordType,
    dns_record_target: draft.dnsRecordTarget.trim(),
    fallback_page_heading: draft.fallbackPageHeading.trim(),
    fallback_page_message: draft.fallbackPageMessage.trim(),
    certificate_provider: draft.certificateProvider,
    custom_certificate_id: draft.customCertificateId,
    concurrent_builds: draft.concurrentBuilds,
  });
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save infrastructure settings.";
    saveState.value = "error";
    toast.error("Could not save infrastructure settings", { description: requestError.value });
    return;
  }
  applySettings(result.data);
  saveState.value = "saved";
  toast.success("Infrastructure settings saved");
}

function resetSettings() {
  if (!savedSettings.value) return;
  Object.assign(draft, cloneSettings(savedSettings.value));
  requestError.value = "";
  saveState.value = "idle";
}

async function loadSettings(showSuccess = false) {
  if (saveState.value === "saving") return;
  saveState.value = "loading";
  requestError.value = "";
  const result = await apiGetInfrastructureSettings();
  if (!result.success) {
    requestError.value = result.error ?? "Unable to load infrastructure settings.";
    saveState.value = "error";
    toast.error("Infrastructure unavailable", { description: requestError.value });
    return;
  }
  applySettings(result.data);
  saveState.value = "idle";
  if (showSuccess) toast.success("Infrastructure settings refreshed");
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
          {{ sectionDescription }}
        </p>
      </div>
      <div class="flex w-full items-center justify-between gap-3 sm:w-auto sm:justify-end">
        <span
          class="flex items-center gap-1.5 font-mono text-[10px] text-muted-foreground"
          role="status"
          aria-live="polite"
        >
          <span v-if="saveState === 'loading'">Loading infrastructure</span>
          <span v-else-if="saveState === 'saving'">Saving infrastructure</span>
          <span v-else-if="isDirty">Unsaved changes</span>
          <span v-else>No unsaved changes</span>
        </span>
        <Button
          v-if="!isDirty"
          variant="outline"
          size="sm"
          type="button"
          :disabled="saveState === 'loading' || saveState === 'saving'"
          @click="loadSettings(true)"
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

    <Tabs :model-value="activeSection" class="mt-6 gap-0" @update:model-value="selectSection">
      <TabsList
        class="h-9 max-w-full justify-start overflow-x-auto rounded-[4px] max-[560px]:w-full"
        aria-label="Infrastructure sections"
      >
        <TabsTrigger value="overview" class="min-w-max px-3 text-xs">
          <LayoutDashboard class="size-3.5" :stroke-width="1.5" />
          Overview
        </TabsTrigger>
        <TabsTrigger value="ingress" class="min-w-max px-3 text-xs">
          <Network class="size-3.5" :stroke-width="1.5" />
          Ingress &amp; TLS
        </TabsTrigger>
        <TabsTrigger value="backup" class="min-w-max px-3 text-xs">
          <HardDriveDownload class="size-3.5" :stroke-width="1.5" />
          Backup
        </TabsTrigger>
      </TabsList>

      <TabsContent value="overview" class="mt-4 grid gap-4">
        <div class="grid gap-4 xl:grid-cols-2">
          <InfrastructureHealth :health="infrastructureHealth" />
          <ApplicationEnvironment :environment="applicationEnvironment" />
        </div>
        <BuildCapacitySettings
          :concurrent-builds="draft.concurrentBuilds"
          :error="isDirty ? buildCapacityError : ''"
          @update:concurrent-builds="
            draft.concurrentBuilds = $event;
            markDirty();
          "
        />
      </TabsContent>

      <TabsContent value="ingress" class="mt-4">
        <form class="grid gap-4" @submit.prevent="saveSettings">
          <ControlPlaneIngressSettings
            :domain="draft.controlPlaneDomain"
            :error="isDirty ? controlPlaneDomainError : ''"
            @update:domain="
              draft.controlPlaneDomain = $event;
              markDirty();
            "
          />

          <ApplicationIngressSettings
            :public-origin="applicationEnvironment?.public_origin ?? ''"
            :control-plane-domain="draft.controlPlaneDomain"
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

          <IngressFallbackSettings
            :heading="draft.fallbackPageHeading"
            :message="draft.fallbackPageMessage"
            :heading-error="isDirty ? fallbackHeadingError : ''"
            :message-error="isDirty ? fallbackMessageError : ''"
            @update:heading="
              draft.fallbackPageHeading = $event;
              markDirty();
            "
            @update:message="
              draft.fallbackPageMessage = $event;
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
      </TabsContent>

      <TabsContent value="backup" class="mt-4">
        <BackupDestinationSettings />
      </TabsContent>
    </Tabs>
  </div>
</template>
