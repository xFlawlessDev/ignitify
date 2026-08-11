import { beforeEach, describe, expect, it } from "vitest";
import { useAiChat } from "./useAiChat";

describe("useAiChat", () => {
  const chat = useAiChat();

  beforeEach(() => {
    chat.closeAiChat();
    chat.consumePendingLogContext();
  });

  it("opens the assistant with a bounded deployment log context", () => {
    chat.askAiAboutLogs("Deployment logs", "service exited with status 1");

    expect(chat.isAiChatOpen.value).toBe(true);
    expect(chat.consumePendingLogContext()).toEqual({
      label: "Deployment logs",
      content: "service exited with status 1",
    });
    expect(chat.pendingLogContext.value).toBeNull();
  });
});
