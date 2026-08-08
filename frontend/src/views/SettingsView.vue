<script setup lang="ts">
import { Check, RotateCcw, Save, Settings2 } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import CertificateManager from "@/components/settings/CertificateManager.vue";
import BuildSettings from "@/components/settings/BuildSettings.vue";
import ServerWebSettings from "@/components/settings/ServerWebSettings.vue";
import type {
  CertificateProvider,
  CustomCertificateSummary,
  CustomCertificateUpload,
} from "@/components/settings/types";
import { Button } from "@/components/ui/button";
import {
  apiCreateServerCertificate,
  apiDeleteServerCertificate,
  apiGetServerSettings,
  apiUpdateServerSettings,
} from "@/lib/api";
import type { ServerSettingsResponse } from "@/lib/api/settings";

interface SettingsDraft {
  serverDomain: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
  concurrentBuilds: number;
}

const defaults: SettingsDraft = {
  serverDomain: "",
  httpsEnabled: false,
  automaticallyProvisionSsl: false,
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

function toDraft(settings: ServerSettingsResponse): SettingsDraft {
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
    serverDomain: settings.server_domain,
    httpsEnabled: settings.https_enabled,
    automaticallyProvisionSsl:
      settings.https_enabled &&
      settings.automatically_provision_ssl &&
      certificateProvider === "lets-encrypt",
    certificateProvider:
      customCertificateId || certificateProvider !== "custom" ? certificateProvider : "none",
    customCertificateId,
    customCertificates,
    concurrentBuilds: settings.concurrent_builds,
  };
}

const draft = reactive<SettingsDraft>(cloneSettings(defaults));
const savedSettings = shallowRef<SettingsDraft | null>(null);
const saveState = shallowRef<"loading" | "idle" | "saving" | "saved" | "error">("loading");
const requestError = shallowRef("");

const domainError = computed(() => {
  const value = draft.serverDomain.trim();
  if (!value) return "Server domain is required.";
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
    return "Automatic SSL provisioning requires Let's Encrypt.";
  }
  if (draft.certificateProvider === "custom") {
    if (!draft.customCertificateId) return "Select a custom certificate for this domain.";
    if (
      !draft.customCertificates.some((certificate) => certificate.id === draft.customCertificateId)
    ) {
      return "The selected custom certificate is no longer available.";
    }
  }
  return "";
});

const buildError = computed(() => {
  if (!Number.isInteger(draft.concurrentBuilds) || draft.concurrentBuilds < 1) {
    return "Use at least 1 concurrent build.";
  }
  if (draft.concurrentBuilds > 32) return "Use no more than 32 concurrent builds.";
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
    !buildError.value,
);

function markDirty() {
  if (saveState.value !== "loading" && saveState.value !== "saving") {
    saveState.value = "idle";
  }
  requestError.value = "";
}

function applySettings(settings: ServerSettingsResponse) {
  const next = toDraft(settings);
  Object.assign(draft, next);
  savedSettings.value = cloneSettings(next);
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
  const result = await apiCreateServerCertificate(
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
  const result = await apiDeleteServerCertificate(certificateId);
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
  const result = await apiUpdateServerSettings({
    server_domain: draft.serverDomain.trim(),
    https_enabled: draft.httpsEnabled,
    automatically_provision_ssl: draft.automaticallyProvisionSsl,
    certificate_provider: draft.certificateProvider,
    custom_certificate_id: draft.customCertificateId,
    concurrent_builds: draft.concurrentBuilds,
  });
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save server settings.";
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

onMounted(async () => {
  const result = await apiGetServerSettings();
  if (!result.success) {
    requestError.value = result.error ?? "Unable to load server settings.";
    saveState.value = "error";
    return;
  }
  applySettings(result.data);
  saveState.value = "idle";
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Settings</h1>
        <p class="mt-2 max-w-[58ch] text-sm leading-5 text-muted-foreground">
          Configure the server domain, HTTPS certificates, and build capacity used by each server.
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
          <span v-else-if="saveState === 'loading'">Loading server settings</span>
          <span v-else-if="saveState === 'saving'">Saving server settings</span>
          <span v-else-if="saveState === 'error'">Settings unavailable</span>
          <span v-else-if="isDirty">Unsaved changes</span>
          <span v-else>No unsaved changes</span>
        </span>
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

    <form class="mt-6 grid gap-4" @submit.prevent="saveSettings">
      <div class="flex items-center gap-2 border-b border-border pb-3">
        <Settings2 class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <p class="ui-label">Server configuration</p>
      </div>

      <ServerWebSettings
        :server-domain="draft.serverDomain"
        :https-enabled="draft.httpsEnabled"
        :automatically-provision-ssl="draft.automaticallyProvisionSsl"
        :certificate-provider="draft.certificateProvider"
        :custom-certificate-id="draft.customCertificateId"
        :custom-certificates="draft.customCertificates"
        :domain-error="isDirty ? domainError : ''"
        :tls-error="isDirty ? tlsError : ''"
        @update:server-domain="
          draft.serverDomain = $event;
          markDirty();
        "
        @update:https-enabled="updateHttpsEnabled"
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

      <BuildSettings
        :concurrent-builds="draft.concurrentBuilds"
        @update:concurrent-builds="
          draft.concurrentBuilds = $event;
          markDirty();
        "
      />

      <p v-if="isDirty && buildError" class="text-[11px] text-destructive" role="alert">
        {{ buildError }}
      </p>
      <footer
        class="flex items-center justify-between gap-4 border-t border-border pt-4 text-[11px] text-muted-foreground max-[560px]:items-start max-[560px]:flex-col"
      >
        <p>
          Settings are applied to managed application routes during worker reconciliation. Uploaded
          certificate and private key material are encrypted at rest.
        </p>
        <span class="shrink-0 font-mono">Admin only</span>
      </footer>
    </form>
  </div>
</template>
