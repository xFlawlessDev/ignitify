<script setup lang="ts">
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { onMounted, onUnmounted, useTemplateRef, watch } from "vue";

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
    theme: { background: "#09090b", foreground: "#e4e4e7" },
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
</script>

<template>
  <div class="min-h-0 flex-1 overflow-hidden p-3 sm:p-4">
    <div
      ref="host"
      class="size-full overflow-hidden [&_.xterm-screen]:max-h-full [&_.xterm-screen]:overflow-hidden [&_.xterm]:h-full"
      :data-status="status"
      @pointerdown="focusTerminal"
    />
  </div>
</template>
