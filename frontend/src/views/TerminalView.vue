<script setup lang="ts">
import { Eraser, Maximize2, Minimize2, RefreshCw, TerminalSquare } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef, useTemplateRef, watch } from "vue";
import { toast } from "vue-sonner";
import PtyTerminal from "@/components/PtyTerminal.vue";
import DestinationSelector from "@/components/runtime/DestinationSelector.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePtyTerminal, type PtyTerminalStatus } from "@/composables/usePtyTerminal";
import { apiStepUp, type RemoteServerSummary } from "@/lib/api";
import { createTerminalSocket } from "@/lib/api/terminal";

const stepUpPassword = shallowRef("");
const stepUpToken = shallowRef("");
const stepUpLoading = shallowRef(false);
const selectedDestinationId = shallowRef("local");
const selectedRemoteServer = shallowRef<RemoteServerSummary | null>(null);
const isLocalDestination = computed(() => selectedDestinationId.value === "local");
const { clear, connect, disconnect, error, id, input, output, resize, status } = usePtyTerminal({
  createSocket: () => createTerminalSocket(stepUpToken.value, selectedDestinationId.value),
});
const terminalFrame = useTemplateRef<HTMLElement>("terminalFrame");

const statusLabel = computed(() => {
  const labels: Record<PtyTerminalStatus, string> = {
    connecting: "Connecting",
    running: "Live",
    exited: "Exited",
    error: "Unavailable",
  };
  return labels[status.value];
});

const statusTone = computed(() => {
  if (status.value === "running") return "healthy";
  if (status.value === "connecting") return "live";
  return undefined;
});

const terminalTarget = computed(() => {
  if (isLocalDestination.value) return "Local host shell";
  const server = selectedRemoteServer.value;
  return server ? `${server.username}@${server.host}` : "Remote host shell";
});

const isFullscreen = shallowRef(false);

function syncFullscreen() {
  isFullscreen.value = document.fullscreenElement === terminalFrame.value;
}

function toggleFullscreen() {
  if (!terminalFrame.value) return;
  if (document.fullscreenElement) {
    void document.exitFullscreen();
  } else {
    void terminalFrame.value.requestFullscreen();
  }
}

async function openTerminal(): Promise<void> {
  stepUpLoading.value = true;
  const result = await apiStepUp(stepUpPassword.value);
  stepUpLoading.value = false;
  if (!result.success) {
    toast.error("Password verification failed", {
      description: result.error ?? "Could not verify your password",
    });
    return;
  }
  stepUpPassword.value = "";
  stepUpToken.value = result.data.access_token;
  toast.success("Terminal session authorized");
  await connect();
}

function handleDestinationChange(server: RemoteServerSummary | null) {
  selectedRemoteServer.value = server;
  disconnect();
  stepUpToken.value = "";
}

watch(error, (message) => {
  if (message) toast.error("Terminal unavailable", { description: message });
});

onMounted(() => {
  document.addEventListener("fullscreenchange", syncFullscreen);
});

onUnmounted(() => {
  document.removeEventListener("fullscreenchange", syncFullscreen);
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div class="min-w-0">
        <p class="ui-label">Host administration</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Terminal</h1>
        <p class="mt-2 text-sm text-muted-foreground">
          Interactive shell for the selected infrastructure destination.
        </p>
      </div>
      <div class="flex w-full flex-wrap items-center justify-end gap-2 sm:w-auto">
        <DestinationSelector
          v-model="selectedDestinationId"
          class="min-w-52 flex-1 sm:flex-none"
          @change="handleDestinationChange"
        />
        <Button class="flex-1 shrink-0 sm:flex-none" variant="outline" @click="openTerminal">
          <RefreshCw class="size-4" :stroke-width="1.5" />
          Reconnect
        </Button>
      </div>
    </header>

    <form class="mt-4 flex flex-wrap items-end gap-3" @submit.prevent="openTerminal">
      <Label class="grid min-w-[min(100%,18rem)] flex-1 gap-2">
        <span class="ui-label">Confirm password</span>
        <Input v-model="stepUpPassword" type="password" autocomplete="current-password" required />
      </Label>
      <Button class="max-[700px]:w-full" :disabled="stepUpLoading">
        {{ stepUpLoading ? "Verifying..." : "Open terminal" }}
      </Button>
    </form>

    <div
      class="mt-4 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground"
      role="status"
      aria-live="polite"
    >
      <span class="status-dot" :data-status="statusTone" aria-hidden="true" />
      <span>{{ statusLabel }}</span>
      <span class="text-border" aria-hidden="true">/</span>
      <span>{{ terminalTarget }}</span>
    </div>

    <section
      ref="terminalFrame"
      class="mt-4 flex min-h-[min(650px,calc(100svh_-_240px))] flex-col overflow-hidden rounded-[10px] border border-border bg-[#09090b] text-[#f4f4f5] fullscreen:min-h-svh fullscreen:border-0 fullscreen:rounded-none max-[700px]:min-h-[min(560px,calc(100svh_-_270px))]"
      :aria-label="`${terminalTarget} terminal`"
    >
      <div
        class="flex min-h-[43px] items-center justify-between gap-3 border-b border-[#27272a] bg-[#18181b] px-2 py-[5px] pl-3.5"
      >
        <div class="flex min-w-0 items-center gap-2">
          <TerminalSquare class="size-4 text-[#a1a1aa]" :stroke-width="1.5" />
          <span class="truncate font-mono text-[11px] text-[#a1a1aa]"
            >{{ terminalTarget }} / shell</span
          >
        </div>
        <div class="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                size="icon-sm"
                variant="ghost"
                class="text-[#d4d4d8] hover:bg-[#27272a] hover:text-[#fafafa]"
                aria-label="Clear terminal"
                title="Clear terminal"
                @click="clear"
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
                class="text-[#d4d4d8] hover:bg-[#27272a] hover:text-[#fafafa]"
                :aria-label="isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'"
                :title="isFullscreen ? 'Exit fullscreen' : 'Enter fullscreen'"
                @click="toggleFullscreen"
              >
                <Minimize2 v-if="isFullscreen" class="size-4" :stroke-width="1.5" />
                <Maximize2 v-else class="size-4" :stroke-width="1.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>{{
              isFullscreen ? "Exit fullscreen" : "Enter fullscreen"
            }}</TooltipContent>
          </Tooltip>
        </div>
      </div>
      <div class="relative flex min-h-0 flex-1">
        <PtyTerminal :id="id" :status="status" :output="output" @input="input" @resize="resize" />
      </div>
    </section>

    <p class="mt-2.5 text-[11px] leading-[1.5] text-muted-foreground">
      Commands run with the permissions of the configured Ignitify destination account.
    </p>
  </div>
</template>
