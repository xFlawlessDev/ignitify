<script setup lang="ts">
import { Background } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { Handle, Position, VueFlow, type Edge, type Node } from "@vue-flow/core";
import "@vue-flow/controls/dist/style.css";
import "@vue-flow/core/dist/style.css";
import { Check, Container, Pencil, Plus, RefreshCw, Server, Trash2, Upload } from "@lucide/vue";
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
const showValidation = shallowRef(false);

const form = reactive({
  name: "",
  host: "",
  port: 22,
  username: "ignitify",
  deployPath: "/srv/ignitify",
  knownHosts: "",
  isDefault: true,
});

const selectedServer = computed(
  () => servers.value.find((server) => server.id === selectedServerId.value) ?? null,
);

// Vue Flow measures and writes each node's dimensions after it mounts. Keep its
// node model writable so that measurement cannot be lost to a readonly computed value.
const flowNodes = shallowRef<Node<FlowNodeData>[]>([]);
const flowEdges = shallowRef<Edge[]>([]);

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
  if (!editingId.value && !privateKeyFile.value) return "An SSH private key file is required.";
  if (!editingId.value && !form.knownHosts.trim()) return "known_hosts is required.";
  return "";
});

function resetForm() {
  form.name = "";
  form.host = "";
  form.port = 22;
  form.username = "ignitify";
  form.deployPath = "/srv/ignitify";
  form.knownHosts = "";
  form.isDefault = servers.value.length === 0;
  editingId.value = null;
  privateKeyFile.value = null;
  privateKeyInputKey.value += 1;
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
  form.knownHosts = "";
  form.isDefault = server.is_default;
  editingId.value = server.id;
  privateKeyFile.value = null;
  privateKeyInputKey.value += 1;
  showValidation.value = false;
  dialogOpen.value = true;
}

function selectServer(serverId: string) {
  selectedServerId.value = serverId;
}

function updatePrivateKey(event: Event) {
  privateKeyFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
}

async function loadServers() {
  loading.value = true;
  requestError.value = "";
  const result = await apiListRemoteServers();
  if (result.success) {
    servers.value = result.data;
    if (!result.data.some((server) => server.id === selectedServerId.value)) {
      selectedServerId.value =
        result.data.find((server) => server.is_default)?.id ?? result.data[0]?.id ?? null;
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
    const privateKey = privateKeyFile.value ? await privateKeyFile.value.text() : undefined;
    const input: RemoteServerInput = {
      name: form.name.trim(),
      host: form.host.trim(),
      port: Number(form.port),
      username: form.username.trim(),
      deploy_path: form.deployPath.trim(),
      private_key: privateKey,
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
    selectedServerId.value = result.data.id;
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
  selectedServerId.value =
    servers.value.find((item) => item.is_default)?.id ?? servers.value[0]?.id ?? null;
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

    <div class="mt-6 grid gap-5 xl:grid-cols-[minmax(0,1fr)_292px]">
      <section class="app-surface overflow-hidden" aria-labelledby="remote-server-map-heading">
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
              <p class="mt-1.5 text-xs leading-5 text-muted-foreground">
                Drag the canvas to inspect destinations. Select a host to manage it.
              </p>
            </div>
          </div>
          <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
            {{ servers.length }} {{ servers.length === 1 ? "HOST" : "HOSTS" }}
          </span>
        </header>

        <div v-if="loading" class="grid h-[520px] place-items-center text-xs text-muted-foreground">
          Loading destinations…
        </div>
        <VueFlow
          v-else
          class="h-[520px] bg-muted/35 [&_.vue-flow__controls-button:last-child]:border-b-0 [&_.vue-flow__controls-button:hover]:bg-muted [&_.vue-flow__controls-button]:size-[18px] [&_.vue-flow__controls-button]:border-b [&_.vue-flow__controls-button]:border-border [&_.vue-flow__controls-button]:bg-card [&_.vue-flow__controls-button]:text-foreground [&_.vue-flow__controls]:overflow-hidden [&_.vue-flow__controls]:rounded-[3px] [&_.vue-flow__controls]:border [&_.vue-flow__controls]:border-border [&_.vue-flow__controls]:shadow-none [&_.vue-flow__edge-path]:stroke-[1.25] [&_.vue-flow__edge-path]:stroke-border [&_.vue-flow__edge-text]:fill-muted-foreground [&_.vue-flow__edge-text]:font-mono [&_.vue-flow__edge-textbg]:fill-card"
          v-model:nodes="flowNodes"
          v-model:edges="flowEdges"
          :min-zoom="0.55"
          :max-zoom="1.4"
          :nodes-draggable="false"
          :nodes-connectable="false"
          :elements-selectable="false"
          :zoom-on-double-click="false"
          :default-viewport="{ x: 0, y: 0, zoom: 1 }"
        >
          <Background :gap="20" :size="1" color="var(--border)" />
          <Controls position="bottom-right" :show-interactive="false" />

          <template #node-origin>
            <div
              class="nodrag nowheel grid w-[258px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border border-border bg-card p-4 text-foreground shadow-none"
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
                @click.stop="addServer"
              >
                <Plus class="size-3.5" :stroke-width="1.5" />
                Add destination
              </Button>
            </div>
          </template>

          <template #node-remote="{ data }">
            <button
              class="nodrag nowheel block w-[258px] rounded-[8px] border border-border bg-card p-4 text-left text-foreground shadow-none transition-[border-color,transform] duration-150 ease-out hover:border-ring focus-visible:border-ring focus-visible:outline-none motion-reduce:transition-none"
              :class="data.server.id === selectedServerId ? 'border-ring' : ''"
              type="button"
              :aria-pressed="data.server.id === selectedServerId"
              @click.stop="selectServer(data.server.id)"
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
      </section>

      <aside
        class="rounded-[10px] border border-border bg-card"
        aria-labelledby="destination-inspector-heading"
      >
        <header class="border-b border-border px-5 py-4">
          <p class="ui-label">Inspector</p>
          <h2 id="destination-inspector-heading" class="mt-1.5 text-base font-medium">
            {{ selectedServer?.name ?? "No destination selected" }}
          </h2>
        </header>

        <div v-if="selectedServer" class="divide-y divide-border">
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
              <dt class="font-mono text-[10px] text-muted-foreground uppercase">Runner</dt>
              <dd class="text-[11px] text-muted-foreground">Not attached</dd>
            </div>
          </dl>
          <div class="grid gap-2 px-5 py-4">
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
        <div v-else class="px-5 py-8 text-xs leading-5 text-muted-foreground">
          Add a destination, then select its card from the topology.
        </div>
      </aside>
    </div>

    <Dialog :open="dialogOpen" @update:open="updateDialog">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">
            {{ editingId ? "Edit remote server" : "Add remote server" }}
          </DialogTitle>
          <DialogDescription class="text-xs leading-5">
            Private keys and host trust records are encrypted before they are stored. Leave secret
            fields empty when editing to preserve their current values.
          </DialogDescription>
        </DialogHeader>

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

          <div
            class="grid gap-4 border-t border-border pt-4 sm:grid-cols-[minmax(0,0.9fr)_minmax(0,1.1fr)]"
          >
            <div class="grid content-start gap-2">
              <Label for="remote-server-key" class="text-xs font-medium">SSH private key</Label>
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
                accept=".pem,.key,.pub"
                @change="updatePrivateKey"
              />
            </div>
            <div class="grid gap-2">
              <Label for="remote-server-known-hosts" class="text-xs font-medium">known_hosts</Label>
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
