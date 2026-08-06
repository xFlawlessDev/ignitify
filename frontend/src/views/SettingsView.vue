<script setup lang="ts">
import { Check, RotateCcw, Save, Settings2 } from "@lucide/vue";
import { computed, reactive, shallowRef } from "vue";
import CertificateManager from "@/components/settings/CertificateManager.vue";
import BuildSettings from "@/components/settings/BuildSettings.vue";
import ServerWebSettings from "@/components/settings/ServerWebSettings.vue";
import type { CertificateProvider, CustomCertificateSummary } from "@/components/settings/types";
import { Button } from "@/components/ui/button";

interface SettingsDraft {
  serverDomain: string;
  httpsEnabled: boolean;
  automaticallyProvisionSsl: boolean;
  certificateProvider: CertificateProvider;
  customCertificateId: string | null;
  customCertificates: CustomCertificateSummary[];
  concurrentBuilds: number;
}

const storageKey = "ignitify.server-settings";
const defaults: SettingsDraft = {
  serverDomain: "",
  httpsEnabled: false,
  automaticallyProvisionSsl: false,
  certificateProvider: "none",
  customCertificateId: null,
  customCertificates: [],
  concurrentBuilds: 2,
};

function readCustomCertificates(value: unknown): CustomCertificateSummary[] {
  if (!Array.isArray(value)) return [];

  return value.flatMap((certificate) => {
    if (!certificate || typeof certificate !== "object") return [];
    const candidate = certificate as Partial<CustomCertificateSummary>;
    if (
      typeof candidate.id !== "string" ||
      typeof candidate.name !== "string" ||
      typeof candidate.certificateFileName !== "string" ||
      typeof candidate.privateKeyFileName !== "string"
    ) {
      return [];
    }
    return [
      {
        id: candidate.id,
        name: candidate.name,
        certificateFileName: candidate.certificateFileName,
        privateKeyFileName: candidate.privateKeyFileName,
      },
    ];
  });
}

function cloneSettings(settings: SettingsDraft): SettingsDraft {
  return {
    ...settings,
    customCertificates: settings.customCertificates.map((certificate) => ({ ...certificate })),
  };
}

function readStoredSettings(): SettingsDraft {
  if (typeof window === "undefined") return cloneSettings(defaults);

  const raw = window.localStorage.getItem(storageKey);
  if (!raw) return cloneSettings(defaults);

  try {
    const stored = JSON.parse(raw) as Partial<SettingsDraft>;
    const concurrentBuilds = Number(stored.concurrentBuilds);
    const customCertificates = readCustomCertificates(stored.customCertificates);
    const certificateProvider: CertificateProvider =
      stored.certificateProvider === "lets-encrypt" || stored.certificateProvider === "custom"
        ? stored.certificateProvider
        : "none";
    const customCertificateId =
      certificateProvider === "custom" &&
      typeof stored.customCertificateId === "string" &&
      customCertificates.some((certificate) => certificate.id === stored.customCertificateId)
        ? stored.customCertificateId
        : null;
    const effectiveCertificateProvider =
      certificateProvider === "custom" && !customCertificateId ? "none" : certificateProvider;

    return {
      serverDomain:
        typeof stored.serverDomain === "string" ? stored.serverDomain : defaults.serverDomain,
      httpsEnabled: typeof stored.httpsEnabled === "boolean" ? stored.httpsEnabled : false,
      automaticallyProvisionSsl:
        stored.certificateProvider === "lets-encrypt" && stored.automaticallyProvisionSsl === true,
      certificateProvider: effectiveCertificateProvider,
      customCertificateId,
      customCertificates,
      concurrentBuilds:
        Number.isInteger(concurrentBuilds) && concurrentBuilds >= 1 && concurrentBuilds <= 32
          ? concurrentBuilds
          : defaults.concurrentBuilds,
    };
  } catch {
    return cloneSettings(defaults);
  }
}

const draft = reactive<SettingsDraft>(readStoredSettings());
const savedSettings = shallowRef<SettingsDraft>(cloneSettings(draft));
const saveState = shallowRef<"idle" | "saved">("idle");

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

const isDirty = computed(() => JSON.stringify(draft) !== JSON.stringify(savedSettings.value));
const canSave = computed(
  () => isDirty.value && !domainError.value && !tlsError.value && !buildError.value,
);

function markDirty() {
  saveState.value = "idle";
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

function addCertificate(certificate: CustomCertificateSummary) {
  draft.customCertificates.push(certificate);
  markDirty();
}

function removeCertificate(certificateId: string) {
  draft.customCertificates = draft.customCertificates.filter(
    (certificate) => certificate.id !== certificateId,
  );
  if (draft.customCertificateId === certificateId) {
    draft.customCertificateId = null;
    draft.certificateProvider = "none";
  }
  markDirty();
}

function saveSettings() {
  if (!canSave.value) return;

  const nextSettings = cloneSettings({
    ...draft,
    serverDomain: draft.serverDomain.trim(),
  });
  Object.assign(draft, nextSettings);
  window.localStorage.setItem(storageKey, JSON.stringify(nextSettings));
  savedSettings.value = cloneSettings(nextSettings);
  saveState.value = "saved";
}

function resetSettings() {
  Object.assign(draft, cloneSettings(savedSettings.value));
  saveState.value = "idle";
}
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[700px]:items-start max-[700px]:flex-col"
    >
      <div>
        <p class="ui-label">Control plane</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Settings</h1>
        <p class="mt-2.5 max-w-[58ch] text-[13px] leading-5 text-muted-foreground">
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
          <span v-if="saveState === 'saved' && !isDirty">Saved locally</span>
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

    <form class="mt-[22px] grid gap-3" @submit.prevent="saveSettings">
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
          Configuration metadata is stored in this browser. Certificate and private key contents
          require the server configuration API.
        </p>
        <span class="shrink-0 font-mono">Admin only</span>
      </footer>
    </form>
  </div>
</template>
