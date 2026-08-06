import { API_BASE } from "../constants";
import { apiGetMe } from "./auth";
import { getToken } from "./core";

const TERMINAL_PROTOCOL = "ignitify-terminal";

export async function createTerminalSocket(): Promise<WebSocket> {
  const actor = await apiGetMe();
  if (!actor.success) {
    throw new Error(actor.error ?? "Could not authenticate the terminal session");
  }

  const token = getToken();
  if (!token) throw new Error("Sign in again to open the host terminal");

  const url = new URL(`${API_BASE}/terminal`, window.location.origin);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  return new WebSocket(url, [TERMINAL_PROTOCOL, `bearer.${token}`]);
}
