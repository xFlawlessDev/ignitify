<script setup lang="ts">
import { Background } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { Handle, Position, useVueFlow, VueFlow, type Edge, type Node } from "@vue-flow/core";
import "@vue-flow/controls/dist/style.css";
import "@vue-flow/core/dist/style.css";
import {
  Check,
  Container,
  Copy,
  Pencil,
  Plus,
  RefreshCw,
  Server,
  Trash2,
  Upload,
  X,
} from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef, watch } from "vue";
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
import { Textarea } from "@/components/ui/textarea";
import {
  apiCheckRemoteServer,
  apiCreateRemoteServer,
  apiDeleteRemoteServer,
  apiListRemoteServers,
  apiSetDefaultRemoteServer,
  apiUpdateRemoteServer,
  type RemoteServerInput,
  type RemoteServerSummary,
} from "@/lib/api";

interface FlowNodeData {
  label?: string;
  server?: RemoteServerSummary;
}

interface ConnectionCheckState {
  serverId: string;
  status: "success" | "error";
  latencyMs?: number;
  message: string;
}

type SecretInputMode = "file" | "text";

const servers = shallowRef<RemoteServerSummary[]>([]);
const selectedServerId = shallowRef<string | null>(null);
const loading = shallowRef(true);
const saving = shallowRef(false);
const removing = shallowRef(false);
const requestError = shallowRef("");
const dialogOpen = shallowRef(false);
const deleteDialogOpen = shallowRef(false);
const serverPendingDeletion = shallowRef<RemoteServerSummary | null>(null);
const editingId = shallowRef<string | null>(null);
const privateKeyFile = shallowRef<File | null>(null);
const privateKeyInputKey = shallowRef(0);
const privateKeyMode = shallowRef<SecretInputMode>("file");
const publicKeyFile = shallowRef<File | null>(null);
const publicKeyInputKey = shallowRef(0);
const publicKeyMode = shallowRef<SecretInputMode>("file");
const showValidation = shallowRef(false);
const checkingServerId = shallowRef<string | null>(null);
const connectionCheck = shallowRef<ConnectionCheckState | null>(null);
const copiedGuideCommand = shallowRef<string | null>(null);

const linuxGuideCommands = {
  generate: 'ssh-keygen -t ed25519 -N "" -f ./ignitify_deploy -C "ignitify-deploy"',
  install:
    "ssh-copy-id -i ./ignitify_deploy.pub {user}@{host}\nchmod 700 ~/.ssh\nchmod 600 ~/.ssh/authorized_keys",
  hostKey: "ssh-keyscan -t ed25519 {host}",
};

const form = reactive({
  name: "",
  host: "",
  port: 22,
  username: "ignitify",
  deployPath: "/srv/ignitify",
  privateKeyText: "",
  publicKeyText: "",
  knownHosts: "",
  isDefault: true,
});

const selectedServer = computed(
  () => servers.value.find((server) => server.id === selectedServerId.value) ?? null,
);
const selectedConnectionCheck = computed(() =>
  connectionCheck.value?.serverId === selectedServerId.value ? connectionCheck.value : null,
);
const privateKeyProvided = computed(() =>
  privateKeyMode.value === "text" ? !!form.privateKeyText.trim() : !!privateKeyFile.value,
);
const publicKeyProvided = computed(() =>
  publicKeyMode.value === "text" ? !!form.publicKeyText.trim() : !!publicKeyFile.value,
);

// Vue Flow measures and writes each node's dimensions after it mounts. Keep its
// node model writable so that measurement cannot be lost to a readonly computed value.
const flowNodes = shallowRef<Node<FlowNodeData>[]>([]);
const flowEdges = shallowRef<Edge[]>([]);
const flow = useVueFlow();

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
    ...servers.value.map((server, index) => ({
      id: server.id,
      type: "remote",
      label: server.name,
      position: { x: 448, y: 84 + index * 176 },
      data: { label: server.name, server },
      draggable: false,
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
    })),
  ];

  flowEdges.value = servers.value.map((server) => ({
    id: `control-plane-${server.id}`,
    source: "control-plane",
    target: server.id,
    type: "smoothstep",
    label: `SSH ${server.port}`,
    labelShowBg: true,
    labelBgPadding: [4, 3],
    labelBgBorderRadius: 3,
    labelStyle: { fontSize: "10px" },
    selectable: false,
    focusable: false,
  }));
}

watch(servers, updateFlowTopology, { immediate: true });

const formError = computed(() => {
  if (!form.name.trim()) return "Server name is required.";
  if (!form.host.trim() || /[\s/@:]/.test(form.host.trim())) {
    return "Enter a hostname or IP address without a port.";
  }
  if (!Number.isInteger(Number(form.port)) || Number(form.port) < 1 || Number(form.port) > 65535) {
    return "SSH port must be between 1 and 65535.";
  }
  if (!/^[a-z_][a-z0-9_-]{0,31}$/.test(form.username.trim())) {
    return "Enter a valid Linux SSH username.";
  }
  if (!form.deployPath.trim().startsWith("/")) {
    return "Deployment path must start with /.";
  }
  if (!editingId.value && !privateKeyProvided.value) return "An SSH private key is required.";
  if (
    editingId.value &&
    selectedServer.value &&
    !selectedServer.value.public_key_configured &&
    !publicKeyProvided.value
  ) {
    return "An SSH public key is required for this server.";
  }
  if (!editingId.value && !publicKeyProvided.value) return "An SSH public key is required.";
  if (!editingId.value && !form.knownHosts.trim()) return "known_hosts is required.";
  return "";
});

function resetForm() {
  form.name = "";
  form.host = "";
  form.port = 22;
  form.username = "ignitify";
  form.deployPath = "/srv/ignitify";
  form.privateKeyText = "";
  form.publicKeyText = "";
  form.knownHosts = "";
  form.isDefault = servers.value.length === 0;
  editingId.value = null;
  privateKeyFile.value = null;
  privateKeyInputKey.value += 1;
  privateKeyMode.value = "file";
  publicKeyFile.value = null;
  publicKeyInputKey.value += 1;
  publicKeyMode.value = "file";
  showValidation.value = false;
}

function updateDialog(open: boolean) {
  dialogOpen.value = open;
  if (!open) resetForm();
}

function addServer() {
  resetForm();
  dialogOpen.value = true;
}

function editServer(server: RemoteServerSummary) {
  form.name = server.name;
  form.host = server.host;
  form.port = server.port;
  form.username = server.username;
  form.deployPath = server.deploy_path;
  form.privateKeyText = "";
  form.publicKeyText = "";
  form.knownHosts = "";
  form.isDefault = server.is_default;
  editingId.value = server.id;
  privateKeyFile.value = null;
  privateKeyInputKey.value += 1;
  privateKeyMode.value = "file";
  publicKeyFile.value = null;
  publicKeyInputKey.value += 1;
  publicKeyMode.value = "file";
  showValidation.value = false;
  dialogOpen.value = true;
}

function selectServer(serverId: string) {
  if (selectedServerId.value !== serverId) connectionCheck.value = null;
  selectedServerId.value = serverId;
}

function closeInspector() {
  selectedServerId.value = null;
  connectionCheck.value = null;
}

flow.onNodeClick(({ node }) => {
  if (node.type === "remote") selectServer(node.id);
});

flow.onPaneClick(closeInspector);

async function checkConnection(server: RemoteServerSummary) {
  checkingServerId.value = server.id;
  connectionCheck.value = null;
  try {
    const result = await apiCheckRemoteServer(server.id);
    connectionCheck.value = result.success
      ? {
          serverId: server.id,
          status: "success",
          latencyMs: result.data.latency_ms,
          message: "SSH connection verified",
        }
      : {
          serverId: server.id,
          status: "error",
          message: result.error ?? "SSH connection failed",
        };
  } catch {
    connectionCheck.value = {
      serverId: server.id,
      status: "error",
      message: "SSH connection check failed",
    };
  } finally {
    checkingServerId.value = null;
  }
}

function updatePrivateKey(event: Event) {
  privateKeyFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
}

function updatePublicKey(event: Event) {
  publicKeyFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
}

async function copyGuideCommand(command: string) {
  if (!navigator.clipboard) return;

  try {
    await navigator.clipboard.writeText(command);
    copiedGuideCommand.value = command;
    window.setTimeout(() => {
      if (copiedGuideCommand.value === command) copiedGuideCommand.value = null;
    }, 1600);
  } catch {
    copiedGuideCommand.value = null;
  }
}

async function loadServers() {
  loading.value = true;
  requestError.value = "";
  const result = await apiListRemoteServers();
  if (result.success) {
    servers.value = result.data;
    if (!result.data.some((server) => server.id === selectedServerId.value)) {
      selectedServerId.value = null;
    }
  } else {
    requestError.value = result.error ?? "Unable to load remote servers.";
  }
  loading.value = false;
}

async function saveServer() {
  showValidation.value = true;
  if (formError.value) return;

  saving.value = true;
  requestError.value = "";
  try {
    const privateKey =
      privateKeyMode.value === "text"
        ? form.privateKeyText.trim() || undefined
        : privateKeyFile.value
          ? await privateKeyFile.value.text()
          : undefined;
    const publicKey =
      publicKeyMode.value === "text"
        ? form.publicKeyText.trim() || undefined
        : publicKeyFile.value
          ? await publicKeyFile.value.text()
          : undefined;
    const input: RemoteServerInput = {
      name: form.name.trim(),
      host: form.host.trim(),
      port: Number(form.port),
      username: form.username.trim(),
      deploy_path: form.deployPath.trim(),
      private_key: privateKey,
      public_key: publicKey,
      known_hosts: form.knownHosts.trim() || undefined,
      is_default: form.isDefault,
    };
    const result = editingId.value
      ? await apiUpdateRemoteServer(editingId.value, input)
      : await apiCreateRemoteServer(input);
    if (!result.success) {
      requestError.value = result.error ?? "Unable to save remote server.";
      return;
    }
    await loadServers();
    updateDialog(false);
  } catch {
    requestError.value = "Unable to read the SSH private key file.";
  } finally {
    saving.value = false;
  }
}

async function setDefault(server: RemoteServerSummary) {
  if (server.is_default) return;
  requestError.value = "";
  const result = await apiSetDefaultRemoteServer(server.id);
  if (!result.success) {
    requestError.value = result.error ?? "Unable to update the default destination.";
    return;
  }
  servers.value = servers.value.map((item) =>
    item.id === result.data.id ? result.data : { ...item, is_default: false },
  );
}

function requestDelete(server: RemoteServerSummary) {
  serverPendingDeletion.value = server;
  deleteDialogOpen.value = true;
}

async function removeServer() {
  const server = serverPendingDeletion.value;
  if (!server) return;
  removing.value = true;
  requestError.value = "";
  const result = await apiDeleteRemoteServer(server.id);
  removing.value = false;
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove remote server.";
    return;
  }
  servers.value = servers.value.filter((item) => item.id !== server.id);
  closeInspector();
  serverPendingDeletion.value = null;
  deleteDialogOpen.value = false;
}

onMounted(loadServers);
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Deployment destinations</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Remote servers</h1>
        <p class="mt-2 max-w-[62ch] text-sm leading-5 text-muted-foreground">
          Register trusted SSH destinations for applications that run outside this host.
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" type="button" :disabled="loading" @click="loadServers">
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Refresh
        </Button>
        <Button size="sm" type="button" @click="addServer">
          <Plus class="size-4" :stroke-width="1.5" />
          Add server
        </Button>
      </div>
    </header>

    <p v-if="requestError" class="mt-4 text-[11px] text-destructive" role="alert">
      {{ requestError }}
    </p>

    <section class="app-surface mt-6 overflow-hidden" aria-labelledby="remote-server-map-heading">
      <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
        <div class="flex min-w-0 items-start gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
          >
            <Server class="size-4" :stroke-width="1.5" />
          </span>
          <div>
            <p class="ui-label">Destination map</p>
            <h2 id="remote-server-map-heading" class="mt-1.5 text-base font-medium">
              SSH topology
            </h2>
          </div>
        </div>
        <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
          {{ servers.length }} {{ servers.length === 1 ? "HOST" : "HOSTS" }}
        </span>
      </header>

      <div class="relative h-[calc(100svh_-_15rem)] min-h-[560px] max-h-[860px]">
        <div v-if="loading" class="grid size-full place-items-center text-xs text-muted-foreground">
          Loading destinations…
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
                <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
              </span>
              <div>
                <p class="font-mono text-[10px] text-muted-foreground uppercase">Control plane</p>
                <p class="mt-1 text-xs font-medium">This Ignitify host</p>
              </div>
              <Button
                v-if="!servers.length"
                variant="outline"
                size="sm"
                class="col-span-full mt-1 w-full"
                type="button"
                @pointerdown.stop
                @mousedown.stop
                @click.stop="addServer"
              >
                <Plus class="size-3.5" :stroke-width="1.5" />
                Add destination
              </Button>
            </div>
          </template>

          <template #node-remote="{ data }">
            <button
              class="nodrag nopan nowheel block w-[258px] rounded-[8px] border border-border bg-card p-4 text-left text-foreground shadow-none transition-[border-color,transform] duration-150 ease-out hover:border-ring focus-visible:border-ring focus-visible:outline-none motion-reduce:transition-none"
              :class="data.server.id === selectedServerId ? 'border-ring' : ''"
              type="button"
              :aria-pressed="data.server.id === selectedServerId"
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
                  <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
                </span>
                <span
                  v-if="data.server.is_default"
                  class="inline-flex items-center gap-1 rounded-[3px] border border-metric-green/40 bg-metric-green/10 px-1.5 py-0.5 font-mono text-[9px] text-metric-green"
                >
                  <Check class="size-3" :stroke-width="1.8" />
                  DEFAULT
                </span>
              </div>
              <p class="mt-3 truncate text-sm font-medium">{{ data.server.name }}</p>
              <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
                {{ data.server.username }}@{{ data.server.host }}:{{ data.server.port }}
              </p>
              <p
                class="mt-3 flex items-center gap-1.5 font-mono text-[9px] text-muted-foreground uppercase"
              >
                <span class="size-1.5 rounded-full bg-metric-green" />
                SSH credentials stored
              </p>
            </button>
          </template>
        </VueFlow>

        <aside
          v-if="selectedServer"
          class="absolute inset-x-3 top-3 z-10 max-h-[calc(100%_-_1.5rem)] overflow-y-auto rounded-[8px] border border-border bg-card/95 backdrop-blur-sm sm:left-auto sm:right-4 sm:w-[320px]"
          aria-labelledby="destination-inspector-heading"
        >
          <header
            class="sticky top-0 z-10 flex items-start justify-between gap-3 border-b border-border bg-card/95 px-4 py-3 backdrop-blur-sm"
          >
            <div class="min-w-0">
              <p class="ui-label">Inspector</p>
              <h2 id="destination-inspector-heading" class="mt-1.5 truncate text-base font-medium">
                {{ selectedServer.name }}
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
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">Connection</dt>
                <dd class="truncate font-mono text-[11px]">
                  {{ selectedServer.username }}@{{ selectedServer.host }}:{{ selectedServer.port }}
                </dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">Deploy path</dt>
                <dd class="truncate font-mono text-[11px]">{{ selectedServer.deploy_path }}</dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">Host trust</dt>
                <dd class="flex items-center gap-1.5 text-[11px]">
                  <Check class="size-3.5 text-metric-green" :stroke-width="1.8" />
                  known_hosts configured
                </dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">SSH identity</dt>
                <dd class="flex items-center gap-1.5 text-[11px]">
                  <Check
                    v-if="selectedServer.public_key_configured"
                    class="size-3.5 text-metric-green"
                    :stroke-width="1.8"
                  />
                  <span
                    class="size-1.5 rounded-full"
                    :class="
                      selectedServer.public_key_configured
                        ? 'bg-metric-green'
                        : 'bg-muted-foreground'
                    "
                  />
                  {{
                    selectedServer.public_key_configured
                      ? "public key configured"
                      : "public key missing"
                  }}
                </dd>
              </div>
              <div class="grid gap-1">
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">Runner</dt>
                <dd class="text-[11px] text-muted-foreground">Not attached</dd>
              </div>
            </dl>
            <div class="grid gap-2 px-5 py-4">
              <Button
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                :disabled="checkingServerId === selectedServer.id"
                @click="checkConnection(selectedServer)"
              >
                <RefreshCw
                  class="size-4"
                  :class="checkingServerId === selectedServer.id ? 'animate-spin' : ''"
                  :stroke-width="1.5"
                />
                {{
                  checkingServerId === selectedServer.id
                    ? "Checking connection"
                    : "Check connection"
                }}
              </Button>
              <p
                v-if="selectedConnectionCheck"
                class="border-l-2 pl-3 text-[11px] leading-4"
                :class="
                  selectedConnectionCheck.status === 'success'
                    ? 'border-metric-green text-metric-green'
                    : 'border-destructive text-destructive'
                "
                role="status"
              >
                <span>{{ selectedConnectionCheck.message }}</span>
                <span
                  v-if="selectedConnectionCheck.latencyMs !== undefined"
                  class="text-muted-foreground"
                >
                  · {{ selectedConnectionCheck.latencyMs }} ms
                </span>
              </p>
              <Button
                v-if="!selectedServer.is_default"
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                @click="setDefault(selectedServer)"
              >
                <Check class="size-4" :stroke-width="1.5" />
                Use as default
              </Button>
              <Button
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                @click="editServer(selectedServer)"
              >
                <Pencil class="size-4" :stroke-width="1.5" />
                Edit configuration
              </Button>
              <Button
                variant="ghost"
                class="w-full text-destructive hover:bg-destructive/10 hover:text-destructive"
                size="sm"
                type="button"
                @click="requestDelete(selectedServer)"
              >
                <Trash2 class="size-4" :stroke-width="1.5" />
                Remove server
              </Button>
            </div>
          </div>
        </aside>
      </div>
    </section>

    <Dialog :open="dialogOpen" @update:open="updateDialog">
      <DialogContent
        class="max-h-[min(90dvh,760px)] overflow-y-auto rounded-[10px] shadow-none sm:max-w-2xl"
      >
        <DialogHeader>
          <DialogTitle class="text-base font-medium">
            {{ editingId ? "Edit remote server" : "Add remote server" }}
          </DialogTitle>
          <DialogDescription class="text-xs leading-5">
            Private keys, public keys, and host trust records are encrypted before they are stored.
            Leave credential fields empty when editing to preserve their current values.
          </DialogDescription>
        </DialogHeader>

        <details class="border-y border-border py-3 text-xs">
          <summary class="cursor-pointer font-medium text-foreground">
            Linux SSH setup guide
          </summary>
          <div
            class="mt-3 grid max-h-[min(42vh,320px)] gap-3 overflow-y-auto pr-1 text-[11px] leading-5 text-muted-foreground"
          >
            <ol class="grid gap-3 pl-4">
              <li>
                <span class="font-medium text-foreground">Create a deploy key</span> on the Ignitify
                host or your workstation. This creates the private key and matching
                <code class="font-mono text-foreground">.pub</code> file. Keep the passphrase empty
                because automated SSH checks cannot prompt for one.
                <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
                  <pre
                    class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
                  ><code>{{ linuxGuideCommands.generate }}</code></pre>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="mt-0.5 size-7 shrink-0 rounded-[4px]"
                    type="button"
                    :aria-label="
                      copiedGuideCommand === linuxGuideCommands.generate
                        ? 'Copied'
                        : 'Copy key generation command'
                    "
                    :title="
                      copiedGuideCommand === linuxGuideCommands.generate ? 'Copied' : 'Copy command'
                    "
                    @click="copyGuideCommand(linuxGuideCommands.generate)"
                  >
                    <Check
                      v-if="copiedGuideCommand === linuxGuideCommands.generate"
                      class="size-3.5 text-metric-green"
                      :stroke-width="1.8"
                    />
                    <Copy v-else class="size-3.5" :stroke-width="1.5" />
                  </Button>
                </div>
              </li>
              <li>
                <span class="font-medium text-foreground">Install the public key</span> on the
                remote Linux account that will run deployments.
                <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
                  <pre
                    class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
                  ><code>{{ linuxGuideCommands.install }}</code></pre>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="mt-0.5 size-7 shrink-0 rounded-[4px]"
                    type="button"
                    :aria-label="
                      copiedGuideCommand === linuxGuideCommands.install
                        ? 'Copied'
                        : 'Copy public key installation command'
                    "
                    :title="
                      copiedGuideCommand === linuxGuideCommands.install ? 'Copied' : 'Copy command'
                    "
                    @click="copyGuideCommand(linuxGuideCommands.install)"
                  >
                    <Check
                      v-if="copiedGuideCommand === linuxGuideCommands.install"
                      class="size-3.5 text-metric-green"
                      :stroke-width="1.8"
                    />
                    <Copy v-else class="size-3.5" :stroke-width="1.5" />
                  </Button>
                </div>
              </li>
              <li>
                <span class="font-medium text-foreground">Pin the server host key</span> before
                connecting. Verify the fingerprint with your provider, then paste this output in the
                <code class="font-mono text-foreground">known_hosts</code> field.
                <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
                  <pre
                    class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
                  ><code>{{ linuxGuideCommands.hostKey }}</code></pre>
                  <Button
                    variant="ghost"
                    size="icon"
                    class="mt-0.5 size-7 shrink-0 rounded-[4px]"
                    type="button"
                    :aria-label="
                      copiedGuideCommand === linuxGuideCommands.hostKey
                        ? 'Copied'
                        : 'Copy host key command'
                    "
                    :title="
                      copiedGuideCommand === linuxGuideCommands.hostKey ? 'Copied' : 'Copy command'
                    "
                    @click="copyGuideCommand(linuxGuideCommands.hostKey)"
                  >
                    <Check
                      v-if="copiedGuideCommand === linuxGuideCommands.hostKey"
                      class="size-3.5 text-metric-green"
                      :stroke-width="1.8"
                    />
                    <Copy v-else class="size-3.5" :stroke-width="1.5" />
                  </Button>
                </div>
              </li>
            </ol>
            <div class="grid gap-1 border-l-2 border-border pl-3">
              <p class="font-medium text-foreground">Field mapping</p>
              <p>
                <code class="font-mono text-foreground">Private key</code>: file without
                <code class="font-mono text-foreground">.pub</code> or its full private-key text.
                <code class="font-mono text-foreground">Public key</code>: matching
                <code class="font-mono text-foreground">.pub</code> line.
                <code class="font-mono text-foreground">known_hosts</code>: remote host key; it is
                different from the client public key.
              </p>
            </div>
          </div>
        </details>

        <form class="grid gap-4" @submit.prevent="saveServer">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="remote-server-name" class="text-xs font-medium">Server name</Label>
              <Input
                id="remote-server-name"
                v-model="form.name"
                class="rounded-[3px]"
                autocomplete="off"
              />
            </div>
            <div class="grid gap-2">
              <Label for="remote-server-host" class="text-xs font-medium">Hostname or IP</Label>
              <Input
                id="remote-server-host"
                v-model="form.host"
                class="rounded-[3px] font-mono text-xs"
                placeholder="deploy.example.com"
                autocomplete="off"
              />
            </div>
          </div>

          <div class="grid gap-4 sm:grid-cols-[110px_minmax(0,1fr)_minmax(0,1fr)]">
            <div class="grid gap-2">
              <Label for="remote-server-port" class="text-xs font-medium">SSH port</Label>
              <Input
                id="remote-server-port"
                v-model.number="form.port"
                class="rounded-[3px] font-mono text-xs"
                type="number"
                min="1"
                max="65535"
                inputmode="numeric"
              />
            </div>
            <div class="grid gap-2">
              <Label for="remote-server-user" class="text-xs font-medium">SSH user</Label>
              <Input
                id="remote-server-user"
                v-model="form.username"
                class="rounded-[3px] font-mono text-xs"
                autocomplete="username"
              />
            </div>
            <div class="grid gap-2">
              <Label for="remote-server-path" class="text-xs font-medium">Deploy path</Label>
              <Input
                id="remote-server-path"
                v-model="form.deployPath"
                class="rounded-[3px] font-mono text-xs"
                placeholder="/srv/ignitify"
                autocomplete="off"
              />
            </div>
          </div>

          <div class="grid gap-4 border-t border-border pt-4 sm:grid-cols-2">
            <div class="grid content-start gap-2">
              <div class="flex items-center justify-between gap-3">
                <Label class="text-xs font-medium">SSH private key</Label>
                <div class="inline-flex rounded-[4px] border border-border p-0.5" role="tablist">
                  <button
                    class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
                    :class="privateKeyMode === 'file' ? 'bg-muted text-foreground' : ''"
                    type="button"
                    role="tab"
                    :aria-selected="privateKeyMode === 'file'"
                    @click="privateKeyMode = 'file'"
                  >
                    File
                  </button>
                  <button
                    class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
                    :class="privateKeyMode === 'text' ? 'bg-muted text-foreground' : ''"
                    type="button"
                    role="tab"
                    :aria-selected="privateKeyMode === 'text'"
                    @click="privateKeyMode = 'text'"
                  >
                    Text
                  </button>
                </div>
              </div>
              <template v-if="privateKeyMode === 'file'">
                <Label
                  for="remote-server-key"
                  class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
                >
                  <Upload class="size-4 shrink-0" :stroke-width="1.5" />
                  <span class="truncate">{{
                    privateKeyFile?.name ?? (editingId ? "Keep current key" : "Choose key file")
                  }}</span>
                </Label>
                <input
                  :key="privateKeyInputKey"
                  id="remote-server-key"
                  class="sr-only"
                  type="file"
                  accept="*/*"
                  @change="updatePrivateKey"
                />
              </template>
              <Textarea
                v-else
                v-model="form.privateKeyText"
                class="min-h-[112px] rounded-[3px] font-mono text-[10px] leading-4"
                placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
            <div class="grid content-start gap-2">
              <div class="flex items-center justify-between gap-3">
                <Label class="text-xs font-medium">SSH public key</Label>
                <div class="inline-flex rounded-[4px] border border-border p-0.5" role="tablist">
                  <button
                    class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
                    :class="publicKeyMode === 'file' ? 'bg-muted text-foreground' : ''"
                    type="button"
                    role="tab"
                    :aria-selected="publicKeyMode === 'file'"
                    @click="publicKeyMode = 'file'"
                  >
                    File
                  </button>
                  <button
                    class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
                    :class="publicKeyMode === 'text' ? 'bg-muted text-foreground' : ''"
                    type="button"
                    role="tab"
                    :aria-selected="publicKeyMode === 'text'"
                    @click="publicKeyMode = 'text'"
                  >
                    Text
                  </button>
                </div>
              </div>
              <template v-if="publicKeyMode === 'file'">
                <Label
                  for="remote-server-public-key"
                  class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
                >
                  <Upload class="size-4 shrink-0" :stroke-width="1.5" />
                  <span class="truncate">{{
                    publicKeyFile?.name ?? (editingId ? "Keep current key" : "Choose .pub file")
                  }}</span>
                </Label>
                <input
                  :key="publicKeyInputKey"
                  id="remote-server-public-key"
                  class="sr-only"
                  type="file"
                  accept=".pub,.txt"
                  @change="updatePublicKey"
                />
              </template>
              <Textarea
                v-else
                v-model="form.publicKeyText"
                class="min-h-[112px] rounded-[3px] font-mono text-[10px] leading-4"
                placeholder="ssh-ed25519 AAAAC3... user@host"
                autocomplete="off"
                spellcheck="false"
              />
            </div>
            <div class="grid gap-2 sm:col-span-2">
              <Label for="remote-server-known-hosts" class="text-xs font-medium"
                >known_hosts (server host key)</Label
              >
              <Textarea
                id="remote-server-known-hosts"
                v-model="form.knownHosts"
                class="min-h-[88px] rounded-[3px] font-mono text-[11px] leading-4"
                :placeholder="
                  editingId
                    ? 'Keep current host trust record'
                    : 'deploy.example.com ssh-ed25519 AAAA...'
                "
                autocomplete="off"
              />
            </div>
          </div>

          <div class="flex items-center justify-between gap-3 border-t border-border pt-4">
            <div>
              <p class="text-xs font-medium">Use as default destination</p>
              <p class="mt-1 text-[11px] text-muted-foreground">
                Marks the primary target when a remote runner is attached.
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
              {{ saving ? "Saving" : editingId ? "Save changes" : "Add server" }}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>

    <Dialog :open="deleteDialogOpen" @update:open="(open) => !open && (deleteDialogOpen = false)">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-md">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">Remove remote server</DialogTitle>
          <DialogDescription class="text-xs leading-5">
            {{ serverPendingDeletion?.name }} and its encrypted SSH credentials will be removed from
            this control plane.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child
            ><Button variant="outline" type="button">Cancel</Button></DialogClose
          >
          <Button variant="destructive" type="button" :disabled="removing" @click="removeServer">
            <Trash2 class="size-4" :stroke-width="1.5" />
            {{ removing ? "Removing" : "Remove server" }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
