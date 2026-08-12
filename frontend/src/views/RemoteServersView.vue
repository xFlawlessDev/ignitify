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
import { shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import RemoteServerSettingsForm from "@/components/remote-servers/RemoteServerSettingsForm.vue";
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
import { useRemoteServers } from "@/composables/useRemoteServers";
import type { RemoteServerSummary } from "@/lib/api";

interface FlowNodeData {
  label?: string;
  server?: RemoteServerSummary;
}

const {
  accessDialogOpen,
  accessSetup,
  addServer,
  agentStatusClass,
  agentStatusLabel,
  checkConnection,
  checkingServerId,
  closeInspector,
  copiedGuideCommand,
  copyGuideCommand,
  deleteDialogOpen,
  dialogOpen,
  editServer,
  editingId,
  form,
  formError,
  installAgent,
  installPublicKeyCommand,
  installingAgentServerId,
  linuxGuideCommands,
  loadServers,
  loading,
  loadingAccessServerId,
  privateKeyFile,
  privateKeyInputKey,
  privateKeyMode,
  publicKeyFile,
  publicKeyInputKey,
  publicKeyMode,
  removeServer,
  removing,
  requestDelete,
  saveServer,
  saving,
  selectedConnectionCheck,
  selectedServer,
  selectedServerId,
  selectServer,
  serverPendingDeletion,
  servers,
  setDefault,
  showAccessSetup,
  showValidation,
  t,
  updateDialog,
  updatePrivateKey,
  updatePublicKey,
} = useRemoteServers();

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

flow.onNodeClick(({ node }) => {
  if (node.type === "remote") selectServer(node.id);
});

flow.onPaneClick(closeInspector);
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
        <Button
          variant="outline"
          size="sm"
          type="button"
          :disabled="loading"
          @click="loadServers(true)"
        >
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Refresh
        </Button>
        <Button size="sm" type="button" @click="addServer">
          <Plus class="size-4" :stroke-width="1.5" />
          Add server
        </Button>
      </div>
    </header>

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
                <span class="size-1.5 rounded-full" :class="agentStatusClass(data.server)" />
                {{ agentStatusLabel(data.server) }}
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
                <dt class="font-mono text-[10px] text-muted-foreground uppercase">
                  Monitoring agent
                </dt>
                <dd class="flex items-center gap-1.5 text-[11px]">
                  <span class="size-1.5 rounded-full" :class="agentStatusClass(selectedServer)" />
                  {{ agentStatusLabel(selectedServer) }}
                </dd>
                <dd
                  v-if="selectedServer.agent?.last_heartbeat_at"
                  class="font-mono text-[10px] text-muted-foreground"
                >
                  Last heartbeat {{ selectedServer.agent.last_heartbeat_at }}
                </dd>
                <dd v-if="selectedServer.agent?.last_error" class="text-[10px] text-destructive">
                  {{ selectedServer.agent.last_error }}
                </dd>
              </div>
            </dl>
            <div class="grid gap-2 px-5 py-4">
              <Button
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                :disabled="loadingAccessServerId === selectedServer.id"
                @click="showAccessSetup(selectedServer)"
              >
                <Copy
                  class="size-4"
                  :class="loadingAccessServerId === selectedServer.id ? 'animate-pulse' : ''"
                  :stroke-width="1.5"
                />
                {{
                  loadingAccessServerId === selectedServer.id
                    ? t("remoteServerOnboarding.loadingAccess")
                    : t("remoteServerOnboarding.showAccess")
                }}
              </Button>
              <Button
                variant="outline"
                class="w-full"
                size="sm"
                type="button"
                :disabled="installingAgentServerId === selectedServer.id"
                @click="installAgent(selectedServer)"
              >
                <Upload
                  class="size-4"
                  :class="installingAgentServerId === selectedServer.id ? 'animate-pulse' : ''"
                  :stroke-width="1.5"
                />
                {{
                  installingAgentServerId === selectedServer.id
                    ? "Provisioning agent"
                    : selectedServer.agent
                      ? "Reinstall monitoring agent"
                      : "Install monitoring agent"
                }}
              </Button>
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
          <DialogDescription v-if="editingId" class="text-xs leading-5">
            Private keys, public keys, and host trust records are encrypted before they are stored.
            Leave credential fields empty when editing to preserve their current values.
          </DialogDescription>
          <DialogDescription v-else class="text-xs leading-5">
            {{ t("remoteServerOnboarding.createDescription") }}
          </DialogDescription>
        </DialogHeader>

        <form v-if="!editingId" class="grid gap-4" @submit.prevent="saveServer">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="remote-server-name" class="text-xs font-medium">{{
                t("remoteServerOnboarding.name")
              }}</Label>
              <Input
                id="remote-server-name"
                v-model="form.name"
                class="rounded-[3px]"
                autocomplete="off"
              />
            </div>
            <div class="grid gap-2">
              <Label for="remote-server-host" class="text-xs font-medium">{{
                t("remoteServerOnboarding.host")
              }}</Label>
              <Input
                id="remote-server-host"
                v-model="form.host"
                class="rounded-[3px] font-mono text-xs"
                placeholder="deploy.example.com"
                autocomplete="off"
              />
            </div>
          </div>
          <div class="grid gap-4 sm:grid-cols-[110px_minmax(0,1fr)]">
            <div class="grid gap-2">
              <Label for="remote-server-port" class="text-xs font-medium">{{
                t("remoteServerOnboarding.port")
              }}</Label>
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
              <Label for="remote-server-user" class="text-xs font-medium">{{
                t("remoteServerOnboarding.user")
              }}</Label>
              <Input
                id="remote-server-user"
                v-model="form.username"
                class="rounded-[3px] font-mono text-xs"
                autocomplete="username"
              />
            </div>
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
              {{
                saving ? t("remoteServerOnboarding.creating") : t("remoteServerOnboarding.create")
              }}
            </Button>
          </DialogFooter>
        </form>

        <RemoteServerSettingsForm
          v-if="editingId"
          :copied-guide-command="copiedGuideCommand"
          :form="form"
          :form-error="formError"
          :linux-guide-commands="linuxGuideCommands"
          :private-key-file="privateKeyFile"
          :private-key-input-key="privateKeyInputKey"
          :private-key-mode="privateKeyMode"
          :public-key-file="publicKeyFile"
          :public-key-input-key="publicKeyInputKey"
          :public-key-mode="publicKeyMode"
          :saving="saving"
          :show-validation="showValidation"
          @copy-guide-command="copyGuideCommand"
          @save="saveServer"
          @update-private-key="updatePrivateKey"
          @update-private-key-mode="(mode) => (privateKeyMode = mode)"
          @update-public-key="updatePublicKey"
          @update-public-key-mode="(mode) => (publicKeyMode = mode)"
        />
      </DialogContent>
    </Dialog>

    <Dialog :open="accessDialogOpen" @update:open="(open) => !open && (accessDialogOpen = false)">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">{{
            t("remoteServerOnboarding.accessTitle")
          }}</DialogTitle>
          <DialogDescription class="text-xs leading-5">
            {{
              t("remoteServerOnboarding.accessDescription", {
                target: `${accessSetup?.server.username}@${accessSetup?.server.host}`,
              })
            }}
          </DialogDescription>
        </DialogHeader>

        <div class="grid gap-4">
          <div class="grid gap-2">
            <div class="flex items-center justify-between gap-3">
              <Label class="text-xs font-medium">{{ t("remoteServerOnboarding.publicKey") }}</Label>
              <Button
                variant="ghost"
                size="icon"
                class="size-7 shrink-0 rounded-[4px]"
                type="button"
                :aria-label="
                  copiedGuideCommand === accessSetup?.publicKey
                    ? t('remoteServerOnboarding.copied')
                    : t('remoteServerOnboarding.copyPublicKey')
                "
                :title="
                  copiedGuideCommand === accessSetup?.publicKey
                    ? t('remoteServerOnboarding.copied')
                    : t('remoteServerOnboarding.copyPublicKey')
                "
                @click="accessSetup && copyGuideCommand(accessSetup.publicKey)"
              >
                <Check
                  v-if="copiedGuideCommand === accessSetup?.publicKey"
                  class="size-3.5 text-metric-green"
                  :stroke-width="1.8"
                />
                <Copy v-else class="size-3.5" :stroke-width="1.5" />
              </Button>
            </div>
            <pre
              class="overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-3 font-mono text-[10px] leading-4 text-foreground"
            ><code>{{ accessSetup?.publicKey }}</code></pre>
          </div>

          <div class="grid gap-2">
            <div class="flex items-center justify-between gap-3">
              <Label class="text-xs font-medium">{{
                t("remoteServerOnboarding.installCommand")
              }}</Label>
              <Button
                variant="ghost"
                size="icon"
                class="size-7 shrink-0 rounded-[4px]"
                type="button"
                :aria-label="
                  copiedGuideCommand === installPublicKeyCommand
                    ? t('remoteServerOnboarding.copied')
                    : t('remoteServerOnboarding.copyInstallCommand')
                "
                :title="
                  copiedGuideCommand === installPublicKeyCommand
                    ? t('remoteServerOnboarding.copied')
                    : t('remoteServerOnboarding.copyInstallCommand')
                "
                @click="copyGuideCommand(installPublicKeyCommand)"
              >
                <Check
                  v-if="copiedGuideCommand === installPublicKeyCommand"
                  class="size-3.5 text-metric-green"
                  :stroke-width="1.8"
                />
                <Copy v-else class="size-3.5" :stroke-width="1.5" />
              </Button>
            </div>
            <pre
              class="max-h-40 overflow-auto rounded-[4px] border border-border bg-muted/50 p-3 font-mono text-[10px] leading-4 text-foreground"
            ><code>{{ installPublicKeyCommand }}</code></pre>
          </div>
        </div>

        <DialogFooter>
          <Button type="button" @click="accessDialogOpen = false">{{
            t("remoteServerOnboarding.done")
          }}</Button>
        </DialogFooter>
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
