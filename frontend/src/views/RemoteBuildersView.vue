<script setup lang="ts">
import { Background } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { Handle, Position, useVueFlow, VueFlow, type Edge, type Node } from "@vue-flow/core";
import "@vue-flow/controls/dist/style.css";
import "@vue-flow/core/dist/style.css";
import { Check, Cpu, Pencil, Plus, RefreshCw, Server, Trash2, Upload, X } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef, watch } from "vue";
import { toast } from "vue-sonner";
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

interface FlowNodeData {
  label?: string;
  builder?: RemoteBuilderSummary;
}

const builders = shallowRef<RemoteBuilderSummary[]>([]);
const selectedBuilderId = shallowRef<string | null>(null);
const loading = shallowRef(true);
const saving = shallowRef(false);
const removing = shallowRef(false);
const requestError = shallowRef("");
const dialogOpen = shallowRef(false);
const deleteDialogOpen = shallowRef(false);
const builderPendingDeletion = shallowRef<RemoteBuilderSummary | null>(null);
const editingId = shallowRef<string | null>(null);
const caFile = shallowRef<File | null>(null);
const clientCertificateFile = shallowRef<File | null>(null);
const clientKeyFile = shallowRef<File | null>(null);
const showValidation = shallowRef(false);
const flowNodes = shallowRef<Node<FlowNodeData>[]>([]);
const flowEdges = shallowRef<Edge[]>([]);
const flow = useVueFlow();

const form = reactive({
  name: "",
  endpoint: "",
  registryRepository: "",
  tlsServerName: "",
  isDefault: true,
});

const selectedBuilder = computed(
  () => builders.value.find((builder) => builder.id === selectedBuilderId.value) ?? null,
);

function updateFlowTopology() {
  flowNodes.value = [
    {
      id: "control-plane",
      type: "origin",
      label: "This Ignitify host",
      position: { x: 70, y: 212 },
      data: { label: "This Ignitify host" },
      draggable: false,
      selectable: false,
      sourcePosition: Position.Right,
    },
    ...builders.value.map((builder, index) => ({
      id: builder.id,
      type: "remote",
      label: builder.name,
      position: { x: 448, y: 84 + index * 176 },
      data: { label: builder.name, builder },
      draggable: false,
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
    })),
  ];

  flowEdges.value = builders.value.map((builder) => ({
    id: `control-plane-${builder.id}`,
    source: "control-plane",
    target: builder.id,
    type: "smoothstep",
    label: "mTLS BuildKit",
    labelShowBg: true,
    labelBgPadding: [4, 3],
    labelBgBorderRadius: 3,
    labelStyle: { fontSize: "10px" },
    selectable: false,
    focusable: false,
  }));
}

watch(builders, updateFlowTopology, { immediate: true });

function selectBuilder(builderId: string) {
  selectedBuilderId.value = builderId;
}

function closeInspector() {
  selectedBuilderId.value = null;
}

flow.onNodeClick(({ node }) => {
  if (node.type === "remote") selectBuilder(node.id);
});

flow.onPaneClick(closeInspector);

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

async function loadBuilders(showSuccess = false): Promise<boolean> {
  loading.value = true;
  requestError.value = "";
  const result = await apiListRemoteBuilders();
  if (result.success) {
    builders.value = result.data;
    if (!result.data.some((builder) => builder.id === selectedBuilderId.value)) {
      selectedBuilderId.value = null;
    }
    if (showSuccess) toast.success("Remote builders refreshed");
    loading.value = false;
    return true;
  }
  requestError.value = result.error ?? "Unable to load remote builders.";
  toast.error("Remote builders unavailable", { description: requestError.value });
  loading.value = false;
  return false;
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
  const wasEditing = Boolean(editingId.value);
  const result = editingId.value
    ? await apiUpdateRemoteBuilder(editingId.value, input)
    : await apiCreateRemoteBuilder(input);
  saving.value = false;
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save remote builder.";
    toast.error("Could not save remote builder", { description: requestError.value });
    return;
  }
  if (!(await loadBuilders())) return;
  updateDialog(false);
  toast.success(wasEditing ? "Remote builder updated" : "Remote builder added", {
    description: input.name,
  });
}

async function setDefault(builder: RemoteBuilderSummary) {
  if (builder.is_default) return;
  requestError.value = "";
  const result = await apiSetDefaultRemoteBuilder(builder.id);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to update the default builder.";
    toast.error("Could not set default builder", { description: requestError.value });
    return;
  }
  builders.value = builders.value.map((item) => ({
    ...item,
    is_default: item.id === result.data.id,
  }));
  toast.success("Default builder updated", { description: builder.name });
}

function requestDelete(builder: RemoteBuilderSummary) {
  builderPendingDeletion.value = builder;
  deleteDialogOpen.value = true;
}

async function removeBuilder() {
  const builder = builderPendingDeletion.value;
  if (!builder) return;
  removing.value = true;
  requestError.value = "";
  const result = await apiDeleteRemoteBuilder(builder.id);
  removing.value = false;
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove remote builder.";
    toast.error("Could not remove remote builder", { description: requestError.value });
    return;
  }
  builders.value = builders.value.filter((item) => item.id !== builder.id);
  closeInspector();
  builderPendingDeletion.value = null;
  deleteDialogOpen.value = false;
  toast.success("Remote builder removed", { description: builder.name });
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
        <Button
          variant="outline"
          size="sm"
          type="button"
          :disabled="loading"
          @click="loadBuilders(true)"
        >
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Refresh
        </Button>
        <Button size="sm" type="button" @click="addBuilder">
          <Plus class="size-4" :stroke-width="1.5" />
          Add builder
        </Button>
      </div>
    </header>

    <section class="app-surface mt-6 overflow-hidden" aria-labelledby="remote-builders-heading">
      <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
        <div class="flex min-w-0 items-start gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
          >
            <Cpu class="size-4" :stroke-width="1.5" />
          </span>
          <div>
            <p class="ui-label">BuildKit fleet</p>
            <h2 id="remote-builders-heading" class="mt-1.5 text-base font-medium">
              Builder topology
            </h2>
          </div>
        </div>
        <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
          {{ builders.length }} {{ builders.length === 1 ? "BUILDER" : "BUILDERS" }}
        </span>
      </header>

      <div class="relative h-[calc(100svh_-_15rem)] min-h-[560px] max-h-[860px]">
        <div v-if="loading" class="grid size-full place-items-center text-xs text-muted-foreground">
          Loading builders…
        </div>
        <VueFlow
          v-else
          class="size-full bg-muted/35 [&_.vue-flow__controls-button:last-child]:border-b-0 [&_.vue-flow__controls-button:hover]:bg-muted [&_.vue-flow__controls-button]:size-[18px] [&_.vue-flow__controls-button]:border-b [&_.vue-flow__controls-button]:border-border [&_.vue-flow__controls-button]:bg-card [&_.vue-flow__controls-button]:text-foreground [&_.vue-flow__controls]:overflow-hidden [&_.vue-flow__controls]:rounded-[3px] [&_.vue-flow__controls]:border [&_.vue-flow__controls]:border-border [&_.vue-flow__controls]:shadow-none [&_.vue-flow__edge-path]:stroke-[1.25] [&_.vue-flow__edge-path]:stroke-border [&_.vue-flow__edge-text]:fill-muted-foreground [&_.vue-flow__edge-text]:font-mono [&_.vue-flow__edge-textbg]:fill-card"
          v-model:nodes="flowNodes"
          v-model:edges="flowEdges"
          :min-zoom="0.55"
          :max-zoom="1.4"
          :nodes-draggable="false"
          :nodes-connectable="false"
          :elements-selectable="false"
          :zoom-on-double-click="false"
          :fit-view-on-init="true"
          :default-viewport="{ x: 0, y: 0, zoom: 1 }"
        >
          <Background :gap="20" :size="1" color="var(--border)" />
          <Controls position="bottom-right" :show-interactive="false" />

          <template #node-origin>
            <div
              class="nodrag nopan nowheel grid w-[258px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border border-border bg-card p-4 text-foreground shadow-none"
            >
              <Handle
                type="source"
                :position="Position.Right"
                class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
              />
              <span
                class="grid size-8 place-items-center rounded-[4px] border border-border bg-muted"
              >
                <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
              </span>
              <div>
                <p class="font-mono text-[10px] text-muted-foreground uppercase">Control plane</p>
                <p class="mt-1 text-xs font-medium">This Ignitify host</p>
              </div>
              <Button
                v-if="!builders.length"
                variant="outline"
                size="sm"
                class="col-span-full mt-1 w-full"
                type="button"
                @pointerdown.stop
                @mousedown.stop
                @click.stop="addBuilder"
              >
                <Plus class="size-3.5" :stroke-width="1.5" />
                Add builder
              </Button>
            </div>
          </template>

          <template #node-remote="{ data }">
            <button
              class="nodrag nopan nowheel block w-[258px] rounded-[8px] border border-border bg-card p-4 text-left text-foreground shadow-none transition-[border-color,transform] duration-150 ease-out hover:border-ring focus-visible:border-ring focus-visible:outline-none motion-reduce:transition-none"
              :class="data.builder.id === selectedBuilderId ? 'border-ring' : ''"
              type="button"
              :aria-pressed="data.builder.id === selectedBuilderId"
              @pointerdown.stop
              @mousedown.stop
            >
              <Handle
                type="target"
                :position="Position.Left"
                class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
              />
              <div class="flex items-start justify-between gap-3">
                <span
                  class="grid size-8 shrink-0 place-items-center rounded-[4px] border border-border bg-muted"
                >
                  <Cpu class="size-4 text-muted-foreground" :stroke-width="1.5" />
                </span>
                <span
                  v-if="data.builder.is_default"
                  class="inline-flex items-center gap-1 rounded-[3px] border border-metric-green/40 bg-metric-green/10 px-1.5 py-0.5 font-mono text-[9px] text-metric-green"
                >
                  <Check class="size-3" :stroke-width="1.8" />
                  DEFAULT
                </span>
              </div>
              <p class="mt-3 truncate text-sm font-medium">{{ data.builder.name }}</p>
              <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
                {{ data.builder.endpoint }}
              </p>
              <p class="mt-3 truncate font-mono text-[9px] text-muted-foreground uppercase">
                {{ data.builder.registry_repository }}
              </p>
            </button>
          </template>
        </VueFlow>

        <aside
          v-if="selectedBuilder"
          class="absolute inset-x-3 top-3 z-10 max-h-[calc(100%_-_1.5rem)] overflow-y-auto rounded-[8px] border border-border bg-card sm:left-auto sm:right-4 sm:w-[320px]"
          aria-labelledby="builder-inspector-heading"
        >
          <header
            class="sticky top-0 z-10 flex items-start justify-between gap-3 border-b border-border bg-card px-4 py-3"
          >
            <div class="min-w-0">
              <p class="ui-label">Inspector</p>
              <h2 id="builder-inspector-heading" class="mt-1.5 truncate text-base font-medium">
                {{ selectedBuilder.name }}
              </h2>
            </div>
            <Button
              variant="ghost"
              size="icon-sm"
              class="shrink-0"
              type="button"
              aria-label="Close inspector"
              title="Close inspector"
              @click="closeInspector"
            >
              <X class="size-4" :stroke-width="1.5" />
            </Button>
          </header>

          <div class="divide-y divide-border">
            <dl class="grid gap-4 px-5 py-4 text-xs">
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">
                  BuildKit endpoint
                </dt>
                <dd class="truncate font-mono text-[11px]">{{ selectedBuilder.endpoint }}</dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">
                  Registry output
                </dt>
                <dd class="truncate font-mono text-[11px]">
                  {{ selectedBuilder.registry_repository }}
                </dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">
                  TLS server name
                </dt>
                <dd class="truncate font-mono text-[11px]">
                  {{ selectedBuilder.tls_server_name ?? "Endpoint hostname" }}
                </dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">Build routing</dt>
                <dd class="flex items-center gap-1.5 text-[11px]">
                  <Check
                    v-if="selectedBuilder.is_default"
                    class="size-3.5 text-metric-green"
                    :stroke-width="1.8"
                  />
                  {{ selectedBuilder.is_default ? "Default builder" : "Available builder" }}
                </dd>
              </div>
            </dl>

            <div class="grid gap-2 px-5 py-4">
              <Button
                v-if="!selectedBuilder.is_default"
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                @click="setDefault(selectedBuilder)"
              >
                <Check class="size-4" :stroke-width="1.5" />
                Use as default
              </Button>
              <Button
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                @click="editBuilder(selectedBuilder)"
              >
                <Pencil class="size-4" :stroke-width="1.5" />
                Edit configuration
              </Button>
              <Button
                variant="ghost"
                class="w-full text-destructive hover:bg-destructive/10 hover:text-destructive"
                size="sm"
                type="button"
                @click="requestDelete(selectedBuilder)"
              >
                <Trash2 class="size-4" :stroke-width="1.5" />
                Remove builder
              </Button>
            </div>
          </div>
        </aside>
      </div>
    </section>

    <Dialog :open="deleteDialogOpen" @update:open="(open) => !open && (deleteDialogOpen = false)">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-md">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">Remove remote builder</DialogTitle>
          <DialogDescription class="text-xs leading-5">
            {{ builderPendingDeletion?.name }} and its encrypted TLS credentials will be removed
            from this control plane.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child
            ><Button variant="outline" type="button">Cancel</Button></DialogClose
          >
          <Button variant="destructive" type="button" :disabled="removing" @click="removeBuilder">
            <Trash2 class="size-4" :stroke-width="1.5" />
            {{ removing ? "Removing" : "Remove builder" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

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
