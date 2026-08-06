import { onUnmounted, readonly, shallowRef } from "vue";
import { createTerminalSocket } from "@/lib/api/terminal";

export type PtyTerminalStatus = "connecting" | "running" | "exited" | "error";

interface TerminalSize {
  cols: number;
  rows: number;
}

let terminalId = 0;
const MAX_OUTPUT_CHUNKS = 512;

function nextTerminalId() {
  terminalId += 1;
  return `host-terminal-${terminalId}`;
}

export function usePtyTerminal() {
  const id = shallowRef(nextTerminalId());
  const status = shallowRef<PtyTerminalStatus>("connecting");
  const output = shallowRef<Uint8Array[]>([]);
  const error = shallowRef<string | null>(null);
  const size = shallowRef<TerminalSize | null>(null);
  let socket: WebSocket | null = null;
  let connectionAttempt = 0;
  let receivedExit = false;

  function resetBuffer() {
    id.value = nextTerminalId();
    output.value = [];
  }

  function appendOutput(data: Uint8Array) {
    const next = [...output.value, data];
    if (next.length > MAX_OUTPUT_CHUNKS) {
      id.value = nextTerminalId();
      output.value = next.slice(-MAX_OUTPUT_CHUNKS);
      return;
    }
    output.value = next;
  }

  function sendResize() {
    if (!socket || socket.readyState !== WebSocket.OPEN || !size.value) return;
    socket.send(JSON.stringify({ type: "resize", ...size.value }));
  }

  async function connect() {
    const attempt = ++connectionAttempt;
    socket?.close();
    socket = null;
    resetBuffer();
    receivedExit = false;
    status.value = "connecting";
    error.value = null;

    let nextSocket: WebSocket;
    try {
      nextSocket = await createTerminalSocket();
    } catch (cause) {
      if (attempt !== connectionAttempt) return;
      status.value = "error";
      error.value = cause instanceof Error ? cause.message : "Could not open the host terminal";
      return;
    }
    if (attempt !== connectionAttempt) {
      nextSocket.close();
      return;
    }

    socket = nextSocket;
    socket.binaryType = "arraybuffer";
    socket.onopen = () => {
      if (socket !== nextSocket) return;
      status.value = "running";
      sendResize();
    };
    socket.onmessage = (event) => {
      if (socket !== nextSocket) return;
      if (typeof event.data === "string") {
        handleControlMessage(event.data);
      } else if (event.data instanceof ArrayBuffer) {
        appendOutput(new Uint8Array(event.data));
      } else if (event.data instanceof Blob) {
        void event.data.arrayBuffer().then((data) => {
          if (socket === nextSocket) appendOutput(new Uint8Array(data));
        });
      }
    };
    socket.onerror = () => {
      if (socket === nextSocket && status.value !== "exited") {
        error.value = "The host terminal connection failed";
      }
    };
    socket.onclose = (event) => {
      if (socket !== nextSocket) return;
      socket = null;
      if (status.value === "error" || receivedExit) return;
      status.value = event.wasClean ? "exited" : "error";
      if (!event.wasClean) error.value = "The host terminal connection closed unexpectedly";
    };
  }

  function handleControlMessage(raw: string) {
    try {
      const message = JSON.parse(raw) as { type?: string; message?: string };
      if (message.type === "exited") {
        receivedExit = true;
        status.value = "exited";
      } else if (message.type === "error") {
        status.value = "error";
        error.value = message.message ?? "The host terminal is unavailable";
      }
    } catch {
      // The PTY stream is binary; unknown text messages are ignored.
    }
  }

  function input(inputId: string, data: string) {
    if (inputId !== id.value || !socket || socket.readyState !== WebSocket.OPEN) return;
    socket.send(new TextEncoder().encode(data));
  }

  function resize(inputId: string, cols: number, rows: number) {
    if (inputId !== id.value) return;
    size.value = { cols, rows };
    sendResize();
  }

  function disconnect() {
    connectionAttempt += 1;
    socket?.close();
    socket = null;
  }

  function clear() {
    resetBuffer();
  }

  onUnmounted(disconnect);

  return {
    id: readonly(id),
    status: readonly(status),
    output: readonly(output),
    error: readonly(error),
    connect,
    disconnect,
    clear,
    input,
    resize,
  };
}
