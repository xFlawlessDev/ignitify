// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { usePtyTerminal } from "./usePtyTerminal";

function createSocket() {
  const close = vi.fn();
  return {
    close,
    socket: {
      binaryType: "",
      close,
      onclose: null,
      onerror: null,
      onmessage: null,
      onopen: null,
      readyState: WebSocket.OPEN,
      send: vi.fn(),
    } as unknown as WebSocket,
  };
}

describe("usePtyTerminal", () => {
  it("resets the active terminal state when disconnected", async () => {
    const { close, socket } = createSocket();
    const terminal = usePtyTerminal({ createSocket: async () => socket });

    await terminal.connect();
    socket.onopen?.(new Event("open"));
    socket.onmessage?.(new MessageEvent("message", { data: new Uint8Array([1]).buffer }));
    const activeId = terminal.id.value;

    terminal.disconnect();

    expect(close).toHaveBeenCalledTimes(1);
    expect(terminal.output.value).toEqual([]);
    expect(terminal.id.value).not.toBe(activeId);
    expect(terminal.status.value).toBe("exited");
    expect(terminal.error.value).toBeNull();
  });
});
