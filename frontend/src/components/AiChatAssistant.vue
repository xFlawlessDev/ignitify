<script setup lang="ts">
import {
  Bot,
  Check,
  Copy,
  LoaderCircle,
  Paperclip,
  RotateCcw,
  Send,
  Sparkles,
  Trash2,
  X,
} from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  ConversationScrollButton,
} from "@/components/ai-elements/conversation";
import {
  Message,
  MessageAction,
  MessageActions,
  MessageContent,
  MessageResponse,
  MessageToolbar,
} from "@/components/ai-elements/message";
import { Suggestion, Suggestions } from "@/components/ai-elements/suggestion";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useAiChat } from "@/composables/useAiChat";
import { apiChatWithAi, type AiChatMessage, type AiLogContext } from "@/lib/api/ai";

interface ChatMessage extends AiChatMessage {
  id: string;
}

const { t } = useI18n();
const { closeAiChat, consumePendingLogContext, isAiChatOpen, openAiChat, pendingLogContext } =
  useAiChat();
const messages = shallowRef<ChatMessage[]>([]);
const activeLogContext = shallowRef<AiLogContext | null>(null);
const composer = shallowRef("");
const error = shallowRef("");
const sending = shallowRef(false);
const copiedId = shallowRef<string | null>(null);
let copyTimer: number | undefined;

const suggestions = computed(() => [
  t("ai.chat.suggestions.diagnose"),
  t("ai.chat.suggestions.nextSteps"),
  t("ai.chat.suggestions.explain"),
]);
const canSend = computed(() => Boolean(composer.value.trim()) && !sending.value);
const lastAssistantMessageId = computed(() => {
  for (let index = messages.value.length - 1; index >= 0; index -= 1) {
    if (messages.value[index]?.role === "assistant") return messages.value[index]?.id ?? null;
  }
  return null;
});

watch(
  pendingLogContext,
  (context) => {
    if (!context) return;
    activeLogContext.value = consumePendingLogContext();
    messages.value = [];
    composer.value = t("ai.chat.logPrompt");
    error.value = "";
  },
  { immediate: true },
);

function messageId() {
  return crypto.randomUUID();
}

function close() {
  closeAiChat();
}

function clearConversation() {
  messages.value = [];
  error.value = "";
}

function removeLogContext() {
  activeLogContext.value = null;
}

async function send() {
  const content = composer.value.trim();
  if (!content || sending.value) return;

  const userMessage: ChatMessage = { id: messageId(), role: "user", content };
  const history = [...messages.value, userMessage];
  messages.value = history;
  composer.value = "";
  await requestAssistant(history);
}

async function requestAssistant(history: ChatMessage[]) {
  error.value = "";
  sending.value = true;
  try {
    const result = await apiChatWithAi({
      messages: history.map(({ role, content: messageContent }) => ({
        role,
        content: messageContent,
      })),
      ...(activeLogContext.value ? { log_context: activeLogContext.value } : {}),
    });
    if (!result.success) {
      error.value = result.error ?? t("ai.chat.unavailable");
      return;
    }
    messages.value = [
      ...history,
      { id: messageId(), role: "assistant", content: result.data.content },
    ];
  } catch {
    error.value = t("ai.chat.unavailable");
  } finally {
    sending.value = false;
  }
}

async function copyMessage(id: string, content: string) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(content);
    } else {
      const input = document.createElement("textarea");
      input.value = content;
      input.setAttribute("readonly", "true");
      input.style.position = "fixed";
      input.style.opacity = "0";
      document.body.append(input);
      input.select();
      document.execCommand("copy");
      input.remove();
    }
  } catch {
    error.value = t("ai.chat.copyFailed");
    return;
  }

  copiedId.value = id;
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copiedId.value = null;
    copyTimer = undefined;
  }, 1_600);
}

async function regenerate(messageIdToReplace: string) {
  if (sending.value || messageIdToReplace !== lastAssistantMessageId.value) return;
  const messageIndex = messages.value.findIndex((message) => message.id === messageIdToReplace);
  if (messageIndex < 1) return;

  const history = messages.value.slice(0, messageIndex);
  messages.value = history;
  await requestAssistant(history);
}

async function applySuggestion(suggestion: string) {
  composer.value = suggestion;
  await send();
}

function handleComposerKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    void send();
  }
}
</script>

<template>
  <Tooltip v-if="!isAiChatOpen">
    <TooltipTrigger as-child>
      <Button
        class="fixed right-5 bottom-5 z-40 size-11 rounded-full border border-border bg-primary text-primary-foreground hover:bg-primary/90"
        size="icon"
        :aria-label="t('ai.chat.open')"
        @click="openAiChat"
      >
        <Sparkles class="size-5" :stroke-width="1.5" />
      </Button>
    </TooltipTrigger>
    <TooltipContent>{{ t("ai.chat.open") }}</TooltipContent>
  </Tooltip>

  <aside
    v-if="isAiChatOpen"
    class="fixed right-5 bottom-5 z-50 flex h-[min(640px,calc(100dvh-2.5rem))] w-[calc(100vw-2.5rem)] max-w-[430px] flex-col overflow-hidden rounded-[10px] border border-border bg-card"
    :aria-label="t('ai.chat.title')"
  >
    <header class="flex min-h-14 items-center justify-between gap-3 border-b border-border px-4">
      <div class="flex min-w-0 items-center gap-2">
        <Bot class="size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <h2 class="truncate text-sm font-medium">{{ t("ai.chat.title") }}</h2>
      </div>
      <MessageActions>
        <MessageAction
          :disabled="messages.length === 0 || sending"
          :tooltip="t('ai.chat.clear')"
          @click="clearConversation"
        >
          <Trash2 class="size-4" :stroke-width="1.5" />
        </MessageAction>
        <MessageAction :tooltip="t('ai.chat.close')" @click="close">
          <X class="size-4" :stroke-width="1.5" />
        </MessageAction>
      </MessageActions>
    </header>

    <div
      v-if="activeLogContext"
      class="flex min-h-10 items-center gap-2 border-b border-border bg-muted/40 px-4 py-2"
    >
      <Paperclip class="size-3.5 shrink-0 text-muted-foreground" :stroke-width="1.5" />
      <span class="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground">
        {{ activeLogContext.label }}
      </span>
      <MessageAction :tooltip="t('ai.chat.removeLogContext')" @click="removeLogContext">
        <X class="size-3.5" :stroke-width="1.5" />
      </MessageAction>
    </div>

    <Conversation class="h-full min-h-0 flex-1">
      <ConversationContent
        :class="[
          'mx-auto w-full max-w-4xl gap-4 px-3 py-4 sm:gap-5 sm:px-4 sm:py-5',
          messages.length === 0 && !sending ? 'min-h-full justify-center' : '',
        ]"
      >
        <ConversationEmptyState
          v-if="messages.length === 0 && !sending"
          :description="t('ai.chat.emptyDescription')"
          :title="t('ai.chat.emptyTitle')"
        >
          <template #icon><Sparkles class="size-5" :stroke-width="1.5" /></template>
        </ConversationEmptyState>

        <template v-else>
          <Message v-for="message in messages" :key="message.id" :from="message.role">
            <MessageContent>
              <MessageResponse
                :content="message.content"
                class="[&_p]:leading-relaxed [&_pre]:max-w-full [&_pre]:overflow-x-auto"
              />
              <MessageActions v-if="!sending" class="mt-2">
                <MessageAction
                  :tooltip="copiedId === message.id ? t('ai.chat.copied') : t('ai.chat.copy')"
                  @click="copyMessage(message.id, message.content)"
                >
                  <Check v-if="copiedId === message.id" class="size-3.5 text-chart-2" />
                  <Copy v-else class="size-3.5" />
                </MessageAction>
                <MessageAction
                  v-if="message.role === 'assistant' && message.id === lastAssistantMessageId"
                  :tooltip="t('ai.chat.regenerate')"
                  @click="regenerate(message.id)"
                >
                  <RotateCcw class="size-3.5" />
                </MessageAction>
              </MessageActions>
            </MessageContent>
          </Message>

          <Message v-if="sending" from="assistant">
            <MessageContent class="flex-row items-center py-1 text-muted-foreground">
              <LoaderCircle class="size-4 animate-spin" :stroke-width="1.5" />
              <span class="text-xs">{{ t("ai.chat.thinking") }}</span>
            </MessageContent>
          </Message>
        </template>
      </ConversationContent>
      <ConversationScrollButton class="bottom-3" />
    </Conversation>

    <div class="border-t border-border p-3">
      <Suggestions v-if="messages.length === 0" class="mb-3">
        <Suggestion
          v-for="suggestion in suggestions"
          :key="suggestion"
          :suggestion="suggestion"
          @click="applySuggestion"
        />
      </Suggestions>
      <p v-if="error" class="mb-2 text-xs text-destructive" role="alert">{{ error }}</p>
      <form class="flex items-end gap-2" @submit.prevent="send">
        <Textarea
          v-model="composer"
          class="min-h-10 max-h-28 resize-y text-sm"
          :placeholder="t('ai.chat.placeholder')"
          rows="1"
          @keydown="handleComposerKeydown"
        />
        <Button size="icon" type="submit" :disabled="!canSend" :aria-label="t('ai.chat.send')">
          <Send class="size-4" :stroke-width="1.5" />
        </Button>
      </form>
      <MessageToolbar class="mt-2 text-[10px] text-muted-foreground">
        <span>{{ t("ai.chat.disclosure") }}</span>
      </MessageToolbar>
    </div>
  </aside>
</template>
