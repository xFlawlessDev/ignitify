<script setup lang="ts">
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { onMounted, onUnmounted, useTemplateRef, watch } from "vue";

const TERMINAL_THEME = {
  background: "#09090b",
  foreground: "#f4f4f5",
  cursor: "#f4f4f5",
  cursorAccent: "#09090b",
  selectionBackground: "#3f3f46",
  black: "#71717a",
  red: "#f87171",
  green: "#a3e635",
  yellow: "#facc15",
  blue: "#60a5fa",
  magenta: "#e879f9",
  cyan: "#67e8f9",
  white: "#e4e4e7",
  brightBlack: "#a1a1aa",
  brightRed: "#fca5a5",
  brightGreen: "#bef264",
  brightYellow: "#fde047",
  brightBlue: "#93c5fd",
  brightMagenta: "#f0abfc",
  brightCyan: "#a5f3fc",
  brightWhite: "#fafafa",
};

const props = defineProps<{
  id: string;
  status: "connecting" | "running" | "exited" | "error";
  output: readonly Uint8Array[];
}>();

const emit = defineEmits<{
  input: [id: string, data: string];
  resize: [id: string, cols: number, rows: number];
}>();

const host = useTemplateRef<HTMLDivElement>("host");
let terminal: Terminal | null = null;
let fitAddon: FitAddon | null = null;
let observer: ResizeObserver | null = null;
let sentOutput = 0;

function fit() {
  if (!terminal || !fitAddon || !host.value?.clientWidth || !host.value.clientHeight) return;
  fitAddon.fit();
  emit("resize", props.id, terminal.cols, terminal.rows);
}

function focusTerminal() {
  terminal?.focus();
}

function clearTerminal() {
  // xterm clears scrollback while keeping the active prompt/command line.
  terminal?.clear();
}

function writeOutput() {
  if (!terminal) return;
  if (props.output.length < sentOutput) {
    terminal.reset();
    sentOutput = 0;
  }
  for (const data of props.output.slice(sentOutput)) terminal.write(data);
  sentOutput = props.output.length;
}

function fitAfterFullscreenChange() {
  requestAnimationFrame(() => {
    fit();
    focusTerminal();
  });
}

onMounted(() => {
  terminal = new Terminal({
    cursorBlink: true,
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
    fontSize: 12,
    scrollback: 5000,
    theme: TERMINAL_THEME,
  });
  fitAddon = new FitAddon();
  terminal.loadAddon(fitAddon);
  terminal.open(host.value!);
  terminal.onData((data) => emit("input", props.id, data));
  observer = new ResizeObserver(fit);
  observer.observe(host.value!);
  document.addEventListener("fullscreenchange", fitAfterFullscreenChange);
  writeOutput();
  fitAfterFullscreenChange();
});

watch(() => props.output, writeOutput, { deep: false });
watch(
  () => props.id,
  () => {
    sentOutput = 0;
    terminal?.reset();
    writeOutput();
    requestAnimationFrame(fit);
  },
);

onUnmounted(() => {
  document.removeEventListener("fullscreenchange", fitAfterFullscreenChange);
  observer?.disconnect();
  terminal?.dispose();
});

defineExpose({ clear: clearTerminal });
</script>

<template>
  <div class="min-h-0 flex-1 overflow-hidden p-3 sm:p-4">
    <div
      ref="host"
      class="size-full overflow-hidden text-[#f4f4f5] [&_.xterm-screen]:max-h-full [&_.xterm-screen]:overflow-hidden [&_.xterm]:h-full"
      :data-status="status"
      @pointerdown="focusTerminal"
    />
  </div>
</template>
