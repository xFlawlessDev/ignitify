import { API_BASE } from "../constants";
import { apiGetMe } from "./auth";
import { getToken } from "./core";

const TERMINAL_PROTOCOL = "ignitify-terminal";

export async function createTerminalSocket(
  stepUpToken: string,
  destination?: string,
): Promise<WebSocket> {
  if (!stepUpToken) throw new Error("Reauthenticate to open the host terminal");
  return createSocket("/terminal", "host terminal", stepUpToken, destination);
}

export async function createContainerTerminalSocket(
  containerId: string,
  destination?: string,
): Promise<WebSocket> {
  return createSocket(
    `/runtime/containers/${encodeURIComponent(containerId)}/terminal`,
    "container terminal",
    undefined,
    destination,
  );
}

async function createSocket(
  path: string,
  name: string,
  stepUpToken?: string,
  destination?: string,
): Promise<WebSocket> {
  const actor = await apiGetMe();
  if (!actor.success) {
    throw new Error(actor.error ?? "Could not authenticate the terminal session");
  }

  const token = getToken();
  if (!token) throw new Error(`Sign in again to open the ${name}`);

  const url = new URL(`${API_BASE}${path}`, window.location.origin);
  if (destination && destination !== "local") {
    url.searchParams.set("destination", destination);
  }
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  const protocols = [TERMINAL_PROTOCOL, `bearer.${token}`];
  if (stepUpToken) protocols.push(`stepup.${stepUpToken}`);
  return new WebSocket(url, protocols);
}
