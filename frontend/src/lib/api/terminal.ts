import { API_BASE } from "../constants";
import { apiGetMe } from "./auth";
import { getToken } from "./core";

const TERMINAL_PROTOCOL = "ignitify-terminal";

export async function createTerminalSocket(): Promise<WebSocket> {
  return createSocket("/terminal", "host terminal");
}

export async function createContainerTerminalSocket(containerId: string): Promise<WebSocket> {
  return createSocket(
    `/runtime/containers/${encodeURIComponent(containerId)}/terminal`,
    "container terminal",
  );
}

async function createSocket(path: string, name: string): Promise<WebSocket> {
  const actor = await apiGetMe();
  if (!actor.success) {
    throw new Error(actor.error ?? "Could not authenticate the terminal session");
  }

  const token = getToken();
  if (!token) throw new Error(`Sign in again to open the ${name}`);

  const url = new URL(`${API_BASE}${path}`, window.location.origin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return new WebSocket(url, [TERMINAL_PROTOCOL, `bearer.${token}`]);
}
