<script setup lang="ts">
import { Eraser, Maximize2, Minimize2, RefreshCw, TerminalSquare } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef, useTemplateRef } from "vue";
import PtyTerminal from "@/components/PtyTerminal.vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePtyTerminal, type PtyTerminalStatus } from "@/composables/usePtyTerminal";
import { apiStepUp } from "@/lib/api";
import { createTerminalSocket } from "@/lib/api/terminal";

const stepUpPassword = shallowRef("");
const stepUpToken = shallowRef("");
const stepUpError = shallowRef<string | null>(null);
const stepUpLoading = shallowRef(false);
const { clear, connect, error, id, input, output, resize, status } = usePtyTerminal({
  createSocket: () => createTerminalSocket(stepUpToken.value),
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
  stepUpError.value = null;
  const result = await apiStepUp(stepUpPassword.value);
  stepUpLoading.value = false;
  if (!result.success) {
    stepUpError.value = result.error ?? "Could not verify your password";
    return;
  }
  stepUpPassword.value = "";
  stepUpToken.value = result.data.access_token;
  await connect();
}

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
          Interactive shell on the control-plane host.
        </p>
      </div>
      <Button class="shrink-0 max-[700px]:w-full" variant="outline" @click="openTerminal">
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Reconnect
      </Button>
    </header>

    <form class="mt-4 flex flex-wrap items-end gap-3" @submit.prevent="openTerminal">
      <Label class="grid min-w-[min(100%,18rem)] flex-1 gap-2">
        <span class="ui-label">Confirm password</span>
        <Input
          v-model="stepUpPassword"
          type="password"
          autocomplete="current-password"
          required
        />
      </Label>
      <Button class="max-[700px]:w-full" :disabled="stepUpLoading">
        {{ stepUpLoading ? "Verifying..." : "Open terminal" }}
      </Button>
      <p v-if="stepUpError" class="basis-full text-sm text-destructive" role="alert">
        {{ stepUpError }}
      </p>
    </form>

    <div
      class="mt-4 flex flex-wrap items-center gap-2 font-mono text-[10px] text-muted-foreground"
      role="status"
      aria-live="polite"
    >
      <span class="status-dot" :data-status="statusTone" aria-hidden="true" />
      <span>{{ statusLabel }}</span>
      <span class="text-border" aria-hidden="true">/</span>
      <span>Local host shell</span>
    </div>

    <section
      ref="terminalFrame"
      class="mt-4 flex min-h-[min(650px,calc(100svh_-_240px))] flex-col overflow-hidden rounded-[10px] border border-border bg-[#09090b] fullscreen:min-h-svh fullscreen:border-0 fullscreen:rounded-none max-[700px]:min-h-[min(560px,calc(100svh_-_270px))]"
      aria-label="Host terminal"
    >
      <div
        class="flex min-h-[43px] items-center justify-between gap-3 border-b border-[#27272a] bg-[#18181b] px-2 py-[5px] pl-3.5"
      >
        <div class="flex min-w-0 items-center gap-2">
          <TerminalSquare class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <span class="truncate font-mono text-[11px] text-muted-foreground">host / shell</span>
        </div>
        <div class="flex items-center gap-1">
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                size="icon-sm"
                variant="ghost"
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
        <div
          v-if="error"
          class="absolute right-3.5 bottom-3 max-w-[min(420px,calc(100%_-_28px))] border border-[#7f1d1d] bg-[#450a0a] px-2.5 py-2 text-[11px] text-[#fecaca]"
          role="alert"
        >
          {{ error }}
        </div>
      </div>
    </section>

    <p class="mt-2.5 text-[11px] leading-[1.5] text-muted-foreground">
      Commands run with the operating-system permissions of the Ignitify service account.
    </p>
  </div>
</template>
