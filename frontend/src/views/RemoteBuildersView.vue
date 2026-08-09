<script setup lang="ts">
import { Check, Cpu, Pencil, Plus, RefreshCw, Server, Trash2, Upload } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  apiCreateRemoteBuilder,
  apiDeleteRemoteBuilder,
  apiListRemoteBuilders,
  apiSetDefaultRemoteBuilder,
  apiUpdateRemoteBuilder,
  type RemoteBuilderInput,
  type RemoteBuilderSummary,
} from "@/lib/api";

const builders = shallowRef<RemoteBuilderSummary[]>([]);
const loading = shallowRef(true);
const saving = shallowRef(false);
const requestError = shallowRef("");
const dialogOpen = shallowRef(false);
const editingId = shallowRef<string | null>(null);
const caFile = shallowRef<File | null>(null);
const clientCertificateFile = shallowRef<File | null>(null);
const clientKeyFile = shallowRef<File | null>(null);
const showValidation = shallowRef(false);

const form = reactive({
  name: "",
  endpoint: "",
  registryRepository: "",
  tlsServerName: "",
  isDefault: true,
});

const formError = computed(() => {
  if (!form.name.trim()) return "Builder name is required.";
  if (!/^tcp:\/\/[^/\s]+:\d+$/.test(form.endpoint.trim())) {
    return "Use a TCP endpoint with an explicit port.";
  }
  if (
    !/^[a-z0-9][a-z0-9.-]*(?::[1-9]\d{0,4})?(?:\/[a-z0-9][a-z0-9._-]*)+$/.test(
      form.registryRepository.trim(),
    )
  ) {
    return "Use a registry hostname and repository path.";
  }
  if (!caFile.value || !clientCertificateFile.value || !clientKeyFile.value) {
    return "CA, client certificate, and client key files are required.";
  }
  return "";
});

function resetForm() {
  form.name = "";
  form.endpoint = "";
  form.registryRepository = "";
  form.tlsServerName = "";
  form.isDefault = builders.value.length === 0;
  caFile.value = null;
  clientCertificateFile.value = null;
  clientKeyFile.value = null;
  editingId.value = null;
  showValidation.value = false;
}

function updateDialog(open: boolean) {
  dialogOpen.value = open;
  if (!open) resetForm();
}

function addBuilder() {
  resetForm();
  dialogOpen.value = true;
}

function editBuilder(builder: RemoteBuilderSummary) {
  form.name = builder.name;
  form.endpoint = builder.endpoint;
  form.registryRepository = builder.registry_repository;
  form.tlsServerName = builder.tls_server_name ?? "";
  form.isDefault = builder.is_default;
  editingId.value = builder.id;
  showValidation.value = false;
  dialogOpen.value = true;
}

function updateFile(kind: "ca" | "certificate" | "key", event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0] ?? null;
  if (kind === "ca") caFile.value = file;
  else if (kind === "certificate") clientCertificateFile.value = file;
  else clientKeyFile.value = file;
}

async function loadBuilders() {
  loading.value = true;
  requestError.value = "";
  const result = await apiListRemoteBuilders();
  if (result.success) builders.value = result.data;
  else requestError.value = result.error ?? "Unable to load remote builders.";
  loading.value = false;
}

async function saveBuilder() {
  showValidation.value = true;
  if (formError.value || !caFile.value || !clientCertificateFile.value || !clientKeyFile.value)
    return;

  saving.value = true;
  requestError.value = "";
  const [caCertificate, clientCertificate, clientKey] = await Promise.all([
    caFile.value.text(),
    clientCertificateFile.value.text(),
    clientKeyFile.value.text(),
  ]);
  const input: RemoteBuilderInput = {
    name: form.name.trim(),
    endpoint: form.endpoint.trim(),
    registry_repository: form.registryRepository.trim(),
    tls_server_name: form.tlsServerName.trim() || null,
    ca_certificate: caCertificate,
    client_certificate: clientCertificate,
    client_key: clientKey,
    is_default: form.isDefault,
  };
  const result = editingId.value
    ? await apiUpdateRemoteBuilder(editingId.value, input)
    : await apiCreateRemoteBuilder(input);
  saving.value = false;
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save remote builder.";
    return;
  }
  await loadBuilders();
  updateDialog(false);
}

async function setDefault(builder: RemoteBuilderSummary) {
  if (builder.is_default) return;
  requestError.value = "";
  const result = await apiSetDefaultRemoteBuilder(builder.id);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to update the default builder.";
    return;
  }
  builders.value = builders.value.map((item) => ({
    ...item,
    is_default: item.id === result.data.id,
  }));
}

async function removeBuilder(builder: RemoteBuilderSummary) {
  requestError.value = "";
  const result = await apiDeleteRemoteBuilder(builder.id);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove remote builder.";
    return;
  }
  builders.value = builders.value.filter((item) => item.id !== builder.id);
}

onMounted(loadBuilders);
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Build infrastructure</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Remote builders</h1>
        <p class="mt-2 max-w-[62ch] text-sm leading-5 text-muted-foreground">
          Offload application builds to a managed BuildKit host. Images are pushed to the selected
          registry and pulled by this Ignitify runtime using their immutable digest.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" type="button" :disabled="loading" @click="loadBuilders">
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Refresh
        </Button>
        <Button size="sm" type="button" @click="addBuilder">
          <Plus class="size-4" :stroke-width="1.5" />
          Add builder
        </Button>
      </div>
    </header>

    <p v-if="requestError" class="mt-4 text-[11px] text-destructive" role="alert">
      {{ requestError }}
    </p>

    <section class="app-surface mt-6" aria-labelledby="remote-builders-heading">
      <header class="app-panel-header flex items-start gap-3 px-5 py-4">
        <span
          class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
        >
          <Cpu class="size-4" :stroke-width="1.5" />
        </span>
        <div>
          <p class="ui-label">BuildKit fleet</p>
          <h2 id="remote-builders-heading" class="mt-1.5 text-base font-medium">
            Configured builders
          </h2>
          <p class="mt-1.5 max-w-[62ch] text-xs leading-5 text-muted-foreground">
            The default builder receives all application source builds. Without a default, builds
            run locally on this host.
          </p>
        </div>
      </header>

      <div v-if="loading" class="px-5 py-6 text-xs text-muted-foreground">Loading builders…</div>
      <div v-else-if="builders.length" class="divide-y divide-border">
        <article v-for="builder in builders" :key="builder.id" class="px-5 py-4">
          <div class="flex items-start gap-3">
            <Server class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <p class="text-sm font-medium">{{ builder.name }}</p>
                <span
                  v-if="builder.is_default"
                  class="inline-flex items-center gap-1 rounded-[3px] border border-metric-green/40 bg-metric-green/10 px-1.5 py-0.5 font-mono text-[10px] text-metric-green"
                >
                  <Check class="size-3" :stroke-width="1.8" />
                  DEFAULT
                </span>
              </div>
              <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground">
                {{ builder.endpoint }}
              </p>
              <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
                Push: {{ builder.registry_repository }}
                <template v-if="builder.tls_server_name">
                  · TLS: {{ builder.tls_server_name }}</template
                >
              </p>
            </div>
            <div class="flex shrink-0 items-center gap-1">
              <Button
                v-if="!builder.is_default"
                variant="ghost"
                class="h-8 px-2 text-xs text-muted-foreground hover:text-foreground"
                type="button"
                @click="setDefault(builder)"
              >
                Use default
              </Button>
              <Button
                variant="ghost"
                class="grid size-8 place-items-center rounded-[3px] text-muted-foreground hover:bg-muted hover:text-foreground"
                type="button"
                :aria-label="`Edit ${builder.name}`"
                :title="`Edit ${builder.name}`"
                @click="editBuilder(builder)"
              >
                <Pencil class="size-4" :stroke-width="1.5" />
              </Button>
              <Button
                variant="ghost"
                class="grid size-8 place-items-center rounded-[3px] text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                type="button"
                :aria-label="`Remove ${builder.name}`"
                :title="`Remove ${builder.name}`"
                @click="removeBuilder(builder)"
              >
                <Trash2 class="size-4" :stroke-width="1.5" />
              </Button>
            </div>
          </div>
        </article>
      </div>
      <div v-else class="flex items-center gap-3 px-5 py-7 text-muted-foreground">
        <Server class="size-4 shrink-0" :stroke-width="1.5" />
        <p class="text-xs">No remote builder configured. Application builds use this host.</p>
      </div>
    </section>

    <Dialog :open="dialogOpen" @update:open="updateDialog">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-xl">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">
            {{ editingId ? "Replace remote builder configuration" : "Add remote builder" }}
          </DialogTitle>
          <DialogDescription class="text-xs leading-5">
            BuildKit must expose a TLS-protected TCP endpoint. The registry must be reachable by
            both BuildKit and the local Ignitify runtime.
          </DialogDescription>
        </DialogHeader>

        <form class="grid gap-4" @submit.prevent="saveBuilder">
          <div class="grid gap-2 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="builder-name" class="text-xs font-medium">Builder name</Label>
              <Input
                id="builder-name"
                v-model="form.name"
                class="rounded-[3px]"
                autocomplete="off"
              />
            </div>
            <div class="grid gap-2">
              <Label for="builder-endpoint" class="text-xs font-medium">BuildKit endpoint</Label>
              <Input
                id="builder-endpoint"
                v-model="form.endpoint"
                class="rounded-[3px] font-mono text-xs"
                placeholder="tcp://builder.internal:1234"
                autocomplete="off"
              />
            </div>
          </div>

          <div class="grid gap-2">
            <Label for="builder-registry" class="text-xs font-medium">Registry repository</Label>
            <Input
              id="builder-registry"
              v-model="form.registryRepository"
              class="rounded-[3px] font-mono text-xs"
              placeholder="registry.example.com/ignitify/builds"
              autocomplete="off"
            />
          </div>

          <div class="grid gap-2">
            <Label for="builder-server-name" class="text-xs font-medium">TLS server name</Label>
            <Input
              id="builder-server-name"
              v-model="form.tlsServerName"
              class="rounded-[3px]"
              placeholder="builder.internal"
              autocomplete="off"
            />
          </div>

          <div class="grid gap-2 sm:grid-cols-3">
            <div class="grid gap-2">
              <Label for="builder-ca" class="text-xs font-medium">CA certificate</Label>
              <Label
                for="builder-ca"
                class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
              >
                <Upload class="size-4 shrink-0" :stroke-width="1.5" />
                <span class="truncate">{{ caFile?.name ?? "Choose PEM" }}</span>
              </Label>
              <input
                id="builder-ca"
                class="sr-only"
                type="file"
                accept=".pem,.crt"
                @change="updateFile('ca', $event)"
              />
            </div>
            <div class="grid gap-2">
              <Label for="builder-client-cert" class="text-xs font-medium"
                >Client certificate</Label
              >
              <Label
                for="builder-client-cert"
                class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
              >
                <Upload class="size-4 shrink-0" :stroke-width="1.5" />
                <span class="truncate">{{ clientCertificateFile?.name ?? "Choose PEM" }}</span>
              </Label>
              <input
                id="builder-client-cert"
                class="sr-only"
                type="file"
                accept=".pem,.crt"
                @change="updateFile('certificate', $event)"
              />
            </div>
            <div class="grid gap-2">
              <Label for="builder-client-key" class="text-xs font-medium">Client key</Label>
              <Label
                for="builder-client-key"
                class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
              >
                <Upload class="size-4 shrink-0" :stroke-width="1.5" />
                <span class="truncate">{{ clientKeyFile?.name ?? "Choose PEM" }}</span>
              </Label>
              <input
                id="builder-client-key"
                class="sr-only"
                type="file"
                accept=".pem,.key"
                @change="updateFile('key', $event)"
              />
            </div>
          </div>

          <div class="flex items-center justify-between gap-3 border-t border-border pt-4">
            <div>
              <p class="text-xs font-medium">Use as default</p>
              <p class="mt-1 text-[11px] text-muted-foreground">
                Routes new application builds to this builder.
              </p>
            </div>
            <Switch :model-value="form.isDefault" @update:model-value="form.isDefault = $event" />
          </div>

          <p v-if="showValidation && formError" class="text-[11px] text-destructive" role="alert">
            {{ formError }}
          </p>

          <DialogFooter>
            <DialogClose as-child
              ><Button variant="outline" type="button">Cancel</Button></DialogClose
            >
            <Button type="submit" :disabled="saving">
              <Server class="size-4" :stroke-width="1.5" />
              {{ saving ? "Saving" : editingId ? "Replace configuration" : "Add builder" }}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </div>
</template>
