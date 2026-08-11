import { shallowRef } from "vue";
import type { AiLogContext } from "@/lib/api/ai";

const MAX_LOG_CONTEXT_BYTES = 60 * 1024;
const isAiChatOpen = shallowRef(false);
const pendingLogContext = shallowRef<AiLogContext | null>(null);

function truncateLogContext(content: string): string {
  const encoder = new TextEncoder();
  if (encoder.encode(content).byteLength <= MAX_LOG_CONTEXT_BYTES) return content;

  const truncated = new TextDecoder().decode(
    encoder.encode(content).slice(0, MAX_LOG_CONTEXT_BYTES - 64),
  );
  return `${truncated}\n[Log context truncated before sending to AI.]`;
}

export function useAiChat() {
  function openAiChat() {
    isAiChatOpen.value = true;
  }

  function closeAiChat() {
    isAiChatOpen.value = false;
  }

  function askAiAboutLogs(label: string, content: string) {
    const normalized = content.trim();
    if (!normalized) return;
    pendingLogContext.value = {
      label: label.trim().slice(0, 200),
      content: truncateLogContext(normalized),
    };
    isAiChatOpen.value = true;
  }

  function consumePendingLogContext(): AiLogContext | null {
    const context = pendingLogContext.value;
    pendingLogContext.value = null;
    return context;
  }

  return {
    isAiChatOpen,
    pendingLogContext,
    openAiChat,
    closeAiChat,
    askAiAboutLogs,
    consumePendingLogContext,
  };
}
