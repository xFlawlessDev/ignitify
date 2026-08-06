<script setup lang="ts">
import { Eraser, Maximize2, Minimize2, RefreshCw, TerminalSquare } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef, useTemplateRef } from "vue";
import PtyTerminal from "@/components/PtyTerminal.vue";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { usePtyTerminal, type PtyTerminalStatus } from "@/composables/usePtyTerminal";

const { clear, connect, error, id, input, output, resize, status } = usePtyTerminal();
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

onMounted(() => {
  document.addEventListener("fullscreenchange", syncFullscreen);
  void connect();
});

onUnmounted(() => {
  document.removeEventListener("fullscreenchange", syncFullscreen);
});
</script>

<template>
  <div class="terminal-page">
    <header class="terminal-page__header">
      <div class="min-w-0">
        <p class="ui-label">Host administration</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Terminal</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Interactive shell on the control-plane host.
        </p>
      </div>
      <Button class="shrink-0" variant="outline" @click="connect">
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Reconnect
      </Button>
    </header>

    <div class="terminal-status" role="status" aria-live="polite">
      <span class="status-dot" :data-status="statusTone" aria-hidden="true" />
      <span>{{ statusLabel }}</span>
      <span class="terminal-status__divider" aria-hidden="true">/</span>
      <span>Local host shell</span>
    </div>

    <section ref="terminalFrame" class="terminal-shell" aria-label="Host terminal">
      <div class="terminal-shell__toolbar">
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
      <div class="terminal-shell__body">
        <PtyTerminal :id="id" :status="status" :output="output" @input="input" @resize="resize" />
        <div v-if="error" class="terminal-shell__error" role="alert">
          {{ error }}
        </div>
      </div>
    </section>

    <p class="terminal-note">
      Commands run with the operating-system permissions of the Ignitify service account.
    </p>
  </div>
</template>

<style scoped>
.terminal-page {
  width: 100%;
  max-width: 1200px;
}

.terminal-page__header {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 24px;
  border-bottom: 1px solid var(--border);
  padding-bottom: 25px;
}

.terminal-status {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 16px;
  color: var(--muted-foreground);
  font-family: var(--font-geist-mono);
  font-size: 10px;
}

.terminal-status__divider {
  color: var(--border);
}

.terminal-shell {
  display: flex;
  min-height: min(650px, calc(100svh - 240px));
  flex-direction: column;
  margin-top: 14px;
  overflow: hidden;
  border: 1px solid var(--border);
  background: #09090b;
}

.terminal-shell:fullscreen {
  min-height: 100svh;
  border: 0;
}

.terminal-shell__toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 43px;
  border-bottom: 1px solid #27272a;
  background: #18181b;
  padding: 5px 8px 5px 14px;
}

.terminal-shell__body {
  position: relative;
  display: flex;
  min-height: 0;
  flex: 1;
}

.terminal-shell__error {
  position: absolute;
  right: 14px;
  bottom: 12px;
  max-width: min(420px, calc(100% - 28px));
  border: 1px solid #7f1d1d;
  background: #450a0a;
  padding: 8px 10px;
  color: #fecaca;
  font-size: 11px;
}

.terminal-note {
  margin-top: 10px;
  color: var(--muted-foreground);
  font-size: 11px;
  line-height: 1.5;
}

@media (max-width: 700px) {
  .terminal-page__header {
    align-items: flex-start;
    flex-direction: column;
    gap: 16px;
  }

  .terminal-page__header > :last-child {
    width: 100%;
  }

  .terminal-shell {
    min-height: min(560px, calc(100svh - 270px));
  }
}
</style>
