import { getCurrentInstance, onUnmounted, shallowRef } from "vue";
import { apiOpenEventStream } from "@/lib/api/core";
import type { DeploymentEvent, DeploymentLog, DeploymentSummary } from "@/lib/types";

interface UseDeploymentStreamOptions {
  channel?: "events" | "logs";
  onEvent?: (event: DeploymentEvent) => void;
  onLog?: (log: DeploymentLog) => void;
  onSnapshot?: (deployment: DeploymentSummary) => void;
}

const MAX_RETRY_MS = 8_000;

export function useDeploymentStream(
  deploymentId: string,
  options: UseDeploymentStreamOptions = {},
) {
  const connected = shallowRef(false);
  const error = shallowRef<string | null>(null);
  let currentDeploymentId = deploymentId;
  let controller: AbortController | null = null;
  let retryTimer: ReturnType<typeof setTimeout> | null = null;
  let lastSequence = 0;
  let attempts = 0;
  let connectionGeneration = 0;

  async function connect(nextDeploymentId = currentDeploymentId) {
    if (nextDeploymentId !== currentDeploymentId) {
      lastSequence = 0;
      attempts = 0;
      error.value = null;
    }
    currentDeploymentId = nextDeploymentId;
    if (!currentDeploymentId) return;
    stop();
    const generation = ++connectionGeneration;
    const connectionController = new AbortController();
    controller = connectionController;
    try {
      const response = await apiOpenEventStream(
        `/deployments/${encodeURIComponent(currentDeploymentId)}/${options.channel ?? "events"}`,
        connectionController.signal,
        lastSequence || undefined,
      );
      if (generation !== connectionGeneration || connectionController.signal.aborted) return;
      if (!response.ok || !response.body) throw new Error(`Stream failed: ${response.status}`);
      connected.value = true;
      error.value = null;
      attempts = 0;
      await read(response.body, connectionController.signal);
      if (generation === connectionGeneration && !connectionController.signal.aborted) reconnect();
    } catch (cause) {
      if (generation !== connectionGeneration || connectionController.signal.aborted) return;
      connected.value = false;
      error.value = cause instanceof Error ? cause.message : "Deployment stream failed";
      reconnect();
    }
  }

  function reconnect() {
    if (retryTimer) return;
    const delay = Math.min(1_000 * 2 ** attempts++, MAX_RETRY_MS);
    retryTimer = setTimeout(() => {
      retryTimer = null;
      void connect();
    }, delay);
  }

  function stop() {
    connectionGeneration += 1;
    controller?.abort();
    controller = null;
    if (retryTimer) clearTimeout(retryTimer);
    retryTimer = null;
    connected.value = false;
  }

  async function read(body: ReadableStream<Uint8Array>, signal: AbortSignal) {
    const reader = body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    try {
      while (!signal.aborted) {
        const { value, done } = await reader.read();
        if (done) return;
        buffer += decoder.decode(value, { stream: true }).replaceAll("\r\n", "\n");
        const chunks = buffer.split("\n\n");
        buffer = chunks.pop() ?? "";
        for (const chunk of chunks) apply(chunk);
      }
    } finally {
      reader.releaseLock();
    }
  }

  function apply(message: string) {
    const lines = message.split("\n");
    const id = Number(lines.find((line) => line.startsWith("id:"))?.slice(3));
    if (Number.isFinite(id) && id <= lastSequence) return;
    const event =
      lines
        .find((line) => line.startsWith("event:"))
        ?.slice(6)
        .trim() ?? "message";
    const raw = lines
      .filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).trimStart())
      .join("\n");
    if (!raw) return;
    const data: unknown = JSON.parse(raw);
    if (Number.isFinite(id)) lastSequence = id;
    if (event === "snapshot") {
      options.onSnapshot?.((data as { deployment: DeploymentSummary }).deployment);
      return;
    }
    if (event === "log") {
      options.onLog?.(data as DeploymentLog);
      return;
    }
    options.onEvent?.({ ...(data as Omit<DeploymentEvent, "kind">), kind: event });
  }

  if (getCurrentInstance()) onUnmounted(stop);
  return { connected, error, connect, stop };
}
