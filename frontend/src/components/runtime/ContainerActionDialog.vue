<script setup lang="ts">
import {
  Check,
  CircleAlert,
  Copy,
  Eraser,
  FileText,
  FolderOpen,
  Network,
  RefreshCw,
  Settings2,
  Terminal,
  Trash2,
  Upload,
} from "@lucide/vue";
import Ansi from "ansi-to-vue3";
import { computed, onUnmounted, shallowRef, watch } from "vue";
import {
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogOverlay,
  AlertDialogPortal,
  AlertDialogRoot,
  AlertDialogTitle,
} from "reka-ui";
import PtyTerminal from "@/components/PtyTerminal.vue";
import type { ContainerActionKey } from "@/components/runtime/container-actions";
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
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePtyTerminal } from "@/composables/usePtyTerminal";
import {
  apiGetRuntimeContainerDetails,
  apiGetRuntimeContainerLogs,
  apiRemoveRuntimeContainer,
  apiUploadRuntimeContainerFile,
} from "@/lib/api/runtime-containers";
import { createContainerTerminalSocket } from "@/lib/api/terminal";
import type { RuntimeContainer, RuntimeContainerDetails } from "@/lib/types";

const props = defineProps<{
  action: ContainerActionKey | null;
  container: RuntimeContainer | null;
}>();

const emit = defineEmits<{
  removed: [];
}>();

const open = defineModel<boolean>("open", { required: true });
const removeOpen = defineModel<boolean>("removeOpen", { required: true });

const actions = {
  logs: {
    label: "View Logs",
    description: "Inspect the latest output emitted by this container.",
    icon: FileText,
  },
  config: {
    label: "View Config",
    description: "Review the runtime metadata currently known for this container.",
    icon: Settings2,
  },
  mounts: {
    label: "View Mounts",
    description: "Inspect filesystems mounted into this container.",
    icon: FolderOpen,
  },
  networks: {
    label: "View Networks",
    description: "Inspect network attachments and published ports.",
    icon: Network,
  },
  terminal: {
    label: "Terminal",
    description: "Open an interactive shell for this container.",
    icon: Terminal,
  },
  upload: {
    label: "Upload File",
    description: "Copy a file from this browser into the container filesystem.",
    icon: Upload,
  },
} as const;

const activeAction = computed(() => {
  if (!props.action || props.action === "remove") return null;
  return actions[props.action];
});
const isTerminalAction = computed(() => props.action === "terminal");
const isUploadAction = computed(() => props.action === "upload");
const details = shallowRef<RuntimeContainerDetails | null>(null);
const logs = shallowRef<string | null>(null);
const actionError = shallowRef<string | null>(null);
const actionMessage = shallowRef<string | null>(null);
const loading = shallowRef(false);
const removing = shallowRef(false);
const selectedFile = shallowRef<File | null>(null);
const uploadPath = shallowRef("/tmp");
const copiedLogs = shallowRef(false);
let requestId = 0;
let copyTimer: number | undefined;

const {
  clear: clearTerminal,
  connect: connectTerminal,
  disconnect: disconnectTerminal,
  error: terminalError,
  id: terminalId,
  input: terminalInput,
  output: terminalOutput,
  resize: resizeTerminal,
  status: terminalStatus,
} = usePtyTerminal({
  createSocket: () => {
    const containerId = props.container?.id;
    if (!containerId)
      return Promise.reject(new Error("Select a container before opening a terminal"));
    return createContainerTerminalSocket(containerId);
  },
  idPrefix: "container-terminal",
  name: "container terminal",
});

watch(
  () => [open.value, props.action, props.container?.id] as const,
  ([isOpen, action, containerId]) => {
    requestId += 1;
    disconnectTerminal();
    details.value = null;
    logs.value = null;
    actionError.value = null;
    actionMessage.value = null;
    selectedFile.value = null;
    uploadPath.value = "/tmp";
    copiedLogs.value = false;
    if (copyTimer !== undefined) {
      window.clearTimeout(copyTimer);
      copyTimer = undefined;
    }
    if (!isOpen || !action || !containerId || action === "remove") return;
    if (action === "terminal") {
      void connectTerminal();
      return;
    }
    if (action !== "upload") void loadActionData(action, containerId);
  },
);

onUnmounted(() => {
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
});

async function loadActionData(
  action: Exclude<ContainerActionKey, "remove" | "terminal" | "upload">,
  containerId: string,
) {
  const activeRequest = requestId;
  loading.value = true;
  if (action === "logs") {
    const result = await apiGetRuntimeContainerLogs(containerId);
    if (activeRequest !== requestId) return;
    if (result.success) logs.value = result.data.logs;
    else actionError.value = result.error ?? "Could not load container logs";
    loading.value = false;
    return;
  }
  const result = await apiGetRuntimeContainerDetails(containerId);
  if (activeRequest !== requestId) return;
  if (result.success) details.value = result.data;
  else actionError.value = result.error ?? "Could not load container details";
  loading.value = false;
}

function selectUploadFile(event: Event) {
  const input = event.target as HTMLInputElement;
  selectedFile.value = input.files?.[0] ?? null;
  actionError.value = null;
  actionMessage.value = null;
}

async function uploadFile() {
  const containerId = props.container?.id;
  const file = selectedFile.value;
  if (!containerId || !file) {
    actionError.value = "Choose a file to upload";
    return;
  }
  loading.value = true;
  actionError.value = null;
  actionMessage.value = null;
  const result = await apiUploadRuntimeContainerFile(containerId, file, uploadPath.value);
  loading.value = false;
  if (!result.success) {
    actionError.value = result.error ?? "Could not upload the file";
    return;
  }
  actionMessage.value = `${file.name} uploaded to ${uploadPath.value}`;
}

async function copyLogs() {
  if (!logs.value) return;
  actionError.value = null;
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(logs.value);
    } else {
      const input = document.createElement("textarea");
      input.value = logs.value;
      input.setAttribute("readonly", "true");
      input.style.position = "fixed";
      input.style.opacity = "0";
      document.body.append(input);
      input.select();
      document.execCommand("copy");
      input.remove();
    }
  } catch {
    actionError.value = "Could not copy log output";
    return;
  }

  copiedLogs.value = true;
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copiedLogs.value = false;
    copyTimer = undefined;
  }, 1_600);
}

async function removeContainer() {
  const containerId = props.container?.id;
  if (!containerId) return;
  removing.value = true;
  actionError.value = null;
  const result = await apiRemoveRuntimeContainer(containerId);
  removing.value = false;
  if (!result.success) {
    actionError.value = result.error ?? "Could not remove the container";
    return;
  }
  removeOpen.value = false;
  emit("removed");
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      class="max-h-[calc(100vh-2rem)] w-[calc(100%-1rem)] overflow-y-auto rounded-md shadow-none"
      :class="isTerminalAction ? 'sm:max-w-4xl' : 'sm:max-w-lg'"
    >
      <template v-if="activeAction && container">
        <DialogHeader>
          <div class="flex items-start gap-3 pr-6">
            <div class="grid size-9 shrink-0 place-items-center border border-border bg-muted">
              <component
                :is="activeAction.icon"
                class="size-4 text-muted-foreground"
                :stroke-width="1.5"
              />
            </div>
            <div class="min-w-0">
              <DialogTitle>{{ activeAction.label }}</DialogTitle>
              <DialogDescription>{{ activeAction.description }}</DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div class="grid gap-4">
          <div class="flex items-center justify-between gap-3 border-b border-border pb-3">
            <div class="min-w-0">
              <p class="text-sm font-medium">{{ container.name }}</p>
              <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
                {{ container.id }}
              </p>
            </div>
            <span class="shrink-0 text-xs text-muted-foreground">{{ container.state }}</span>
          </div>

          <p
            v-if="actionError"
            class="border border-destructive/40 px-3 py-2 text-xs text-destructive"
            role="alert"
          >
            {{ actionError }}
          </p>

          <div
            v-if="loading && !isUploadAction"
            class="py-8 text-center text-xs text-muted-foreground"
            role="status"
          >
            Loading container data...
          </div>

          <div v-else-if="action === 'logs'" class="grid gap-2">
            <div class="flex items-center justify-between gap-3">
              <p class="ui-label">Latest output</p>
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    :aria-label="copiedLogs ? 'Copied log output' : 'Copy log output'"
                    :disabled="!logs"
                    @click="copyLogs"
                  >
                    <Check v-if="copiedLogs" class="size-4" :stroke-width="1.5" />
                    <Copy v-else class="size-4" :stroke-width="1.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{{ copiedLogs ? "Copied" : "Copy log output" }}</TooltipContent>
              </Tooltip>
            </div>
            <pre
              class="max-h-96 min-h-28 overflow-auto border border-border bg-muted/40 p-3 font-mono text-xs leading-5 text-muted-foreground"
            ><Ansi>{{ logs || "No log output returned." }}</Ansi></pre>
          </div>

          <dl v-else-if="action === 'config' && details" class="grid gap-3 text-sm">
            <div
              class="grid gap-1 border-b border-border pb-3 sm:grid-cols-[8rem_1fr] sm:items-baseline"
            >
              <dt class="text-xs text-muted-foreground">Image</dt>
              <dd class="truncate font-mono text-xs" :title="details.image">
                {{ details.image || "Unavailable" }}
              </dd>
            </div>
            <div
              class="grid gap-1 border-b border-border pb-3 sm:grid-cols-[8rem_1fr] sm:items-baseline"
            >
              <dt class="text-xs text-muted-foreground">Command</dt>
              <dd class="break-all font-mono text-xs">
                {{ details.config.command.join(" ") || "Image default" }}
              </dd>
            </div>
            <div
              class="grid gap-1 border-b border-border pb-3 sm:grid-cols-[8rem_1fr] sm:items-baseline"
            >
              <dt class="text-xs text-muted-foreground">Entrypoint</dt>
              <dd class="break-all font-mono text-xs">
                {{ details.config.entrypoint.join(" ") || "Image default" }}
              </dd>
            </div>
            <div
              class="grid gap-1 border-b border-border pb-3 sm:grid-cols-[8rem_1fr] sm:items-baseline"
            >
              <dt class="text-xs text-muted-foreground">User</dt>
              <dd class="font-mono text-xs">{{ details.config.user || "Image default" }}</dd>
            </div>
            <div
              class="grid gap-1 border-b border-border pb-3 sm:grid-cols-[8rem_1fr] sm:items-baseline"
            >
              <dt class="text-xs text-muted-foreground">Working directory</dt>
              <dd class="font-mono text-xs">{{ details.config.working_dir || "Image default" }}</dd>
            </div>
            <div class="grid gap-1 sm:grid-cols-[8rem_1fr] sm:items-baseline">
              <dt class="text-xs text-muted-foreground">Environment keys</dt>
              <dd class="break-all font-mono text-xs">
                {{ details.config.environment_keys.join(", ") || "None" }}
              </dd>
            </div>
          </dl>

          <div v-else-if="action === 'mounts' && details" class="grid gap-2">
            <p class="ui-label">Mount points</p>
            <div v-if="details.mounts.length" class="grid gap-2">
              <div
                v-for="mount in details.mounts"
                :key="`${mount.source}-${mount.destination}`"
                class="border border-border bg-muted/40 px-3 py-2"
              >
                <p class="font-mono text-xs">{{ mount.destination || "Unknown destination" }}</p>
                <p class="mt-1 break-all font-mono text-[10px] text-muted-foreground">
                  {{ mount.source || mount.kind || "Unnamed mount" }} /
                  {{ mount.read_only ? "read-only" : "read-write" }}
                </p>
              </div>
            </div>
            <p v-else class="border border-border bg-muted/40 p-3 text-xs text-muted-foreground">
              No mounts reported.
            </p>
          </div>

          <div v-else-if="action === 'networks' && details" class="grid gap-2">
            <p class="ui-label">Network attachments</p>
            <div v-if="details.networks.length" class="grid gap-2">
              <div
                v-for="network in details.networks"
                :key="network.name"
                class="border border-border bg-muted/40 px-3 py-2"
              >
                <p class="font-mono text-xs">{{ network.name }}</p>
                <p class="mt-1 font-mono text-[10px] text-muted-foreground">
                  {{ network.ip_address || "No address" }} / {{ network.gateway || "No gateway" }}
                </p>
              </div>
            </div>
            <p v-else class="border border-border bg-muted/40 p-3 text-xs text-muted-foreground">
              No network attachments reported.
            </p>
          </div>

          <section
            v-else-if="action === 'terminal'"
            class="flex h-[min(520px,60svh)] min-h-80 flex-col overflow-hidden border border-border bg-[#09090b]"
            aria-label="Container terminal"
          >
            <div
              class="flex min-h-10 items-center justify-between gap-3 border-b border-[#27272a] bg-[#18181b] px-2 py-1 pl-3"
            >
              <div class="flex min-w-0 items-center gap-2">
                <Terminal class="size-4 text-muted-foreground" :stroke-width="1.5" />
                <span class="truncate font-mono text-[11px] text-muted-foreground"
                  >{{ container.name }} / shell</span
                >
              </div>
              <div class="flex items-center gap-1">
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label="Clear terminal"
                      title="Clear terminal"
                      @click="clearTerminal"
                    >
                      <Eraser class="size-4" :stroke-width="1.5" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Clear terminal</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      aria-label="Reconnect terminal"
                      title="Reconnect terminal"
                      @click="connectTerminal"
                    >
                      <RefreshCw class="size-4" :stroke-width="1.5" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Reconnect terminal</TooltipContent>
                </Tooltip>
              </div>
            </div>
            <div class="relative flex min-h-0 flex-1">
              <PtyTerminal
                :id="terminalId"
                :status="terminalStatus"
                :output="terminalOutput"
                @input="terminalInput"
                @resize="resizeTerminal"
              />
              <p
                v-if="terminalError"
                class="absolute right-3 bottom-3 max-w-[calc(100%-1.5rem)] border border-[#7f1d1d] bg-[#450a0a] px-2.5 py-2 text-[11px] text-[#fecaca]"
                role="alert"
              >
                {{ terminalError }}
              </p>
            </div>
          </section>

          <div v-else-if="action === 'upload'" class="grid gap-3">
            <label class="grid gap-2 text-sm font-medium" for="container-upload-file">
              Choose a file
              <input
                id="container-upload-file"
                class="block w-full rounded-sm border border-input bg-background px-3 py-2 text-xs file:mr-3 file:border-0 file:bg-transparent file:text-xs file:font-medium"
                type="file"
                @change="selectUploadFile"
              />
            </label>
            <label class="grid gap-2 text-sm font-medium" for="container-upload-path">
              Destination directory
              <input
                id="container-upload-path"
                v-model="uploadPath"
                class="w-full rounded-sm border border-input bg-background px-3 py-2 font-mono text-xs"
                type="text"
              />
            </label>
            <p v-if="selectedFile" class="text-xs text-muted-foreground">
              {{ selectedFile.name }} / {{ selectedFile.size.toLocaleString() }} bytes
            </p>
            <p
              v-if="actionMessage"
              class="border border-border px-3 py-2 text-xs text-muted-foreground"
              role="status"
            >
              {{ actionMessage }}
            </p>
          </div>
        </div>

        <DialogFooter>
          <Button
            v-if="isUploadAction"
            class="w-full sm:w-auto"
            :disabled="loading || !selectedFile"
            @click="uploadFile"
          >
            <Upload class="size-4" :stroke-width="1.5" />
            {{ loading ? "Uploading" : "Upload file" }}
          </Button>
          <DialogClose as-child>
            <Button class="w-full sm:w-auto" variant="outline">Close</Button>
          </DialogClose>
        </DialogFooter>
      </template>
    </DialogContent>
  </Dialog>

  <AlertDialogRoot v-model:open="removeOpen">
    <AlertDialogPortal>
      <AlertDialogOverlay class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm" />
      <AlertDialogContent
        class="fixed top-1/2 left-1/2 z-50 grid w-[calc(100%-2rem)] max-w-lg -translate-x-1/2 -translate-y-1/2 gap-5 rounded-md border bg-background p-6 shadow-none"
      >
        <div class="flex items-start gap-3">
          <div
            class="grid size-9 shrink-0 place-items-center border border-destructive/30 bg-destructive/10 text-destructive"
          >
            <CircleAlert class="size-4" :stroke-width="1.5" />
          </div>
          <div class="min-w-0">
            <AlertDialogTitle class="text-base font-medium">Remove container?</AlertDialogTitle>
            <AlertDialogDescription class="mt-2 text-sm leading-5"
              >This will remove
              <span class="font-medium text-foreground">{{ container?.name }}</span> from the host.
              This action cannot be undone.</AlertDialogDescription
            >
          </div>
        </div>
        <p
          v-if="actionError"
          class="border border-destructive/40 px-3 py-2 text-xs text-destructive"
          role="alert"
        >
          {{ actionError }}
        </p>
        <div class="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <AlertDialogCancel as-child>
            <Button class="w-full sm:w-auto" variant="outline" :disabled="removing">Cancel</Button>
          </AlertDialogCancel>
          <AlertDialogAction as-child>
            <Button
              class="w-full sm:w-auto"
              variant="destructive"
              :disabled="removing || !container"
              @click.prevent="removeContainer"
            >
              <Trash2 class="size-4" :stroke-width="1.5" />
              {{ removing ? "Removing" : "Remove container" }}
            </Button>
          </AlertDialogAction>
        </div>
      </AlertDialogContent>
    </AlertDialogPortal>
  </AlertDialogRoot>
</template>
