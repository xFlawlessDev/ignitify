// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ open: vi.fn() }));

vi.mock("@/lib/api/core", () => ({ apiOpenEventStream: api.open }));

function response(chunks: string[]) {
  const encoder = new TextEncoder();
  return {
    ok: true,
    body: new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
        controller.close();
      },
    }),
  } as Response;
}

afterEach(() => {
  api.open.mockReset();
  vi.resetModules();
});

describe("useDeploymentStream", () => {
  it("dedupes fragmented events on reconnect", async () => {
    api.open.mockResolvedValue(
      response([
        'id: 2\nevent: deployment.running\ndata: {"sequence":2,"deployment_id":"d",',
        '"created_at":"now","payload":{}}\n\n',
      ]),
    );
    const events: string[] = [];
    const { useDeploymentStream } = await import("./useDeploymentStream");
    const stream = useDeploymentStream("d", { onEvent: (event) => events.push(event.kind) });

    await stream.connect();
    await new Promise((resolve) => setTimeout(resolve));

    expect(events).toEqual(["deployment.running"]);
    expect(api.open.mock.calls[0]?.[0]).toBe("/deployments/d/events");
    expect(api.open.mock.calls[0]?.[2]).toBeUndefined();
    stream.stop();
  });
});
