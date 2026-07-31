import type { AuthSession } from "../types";
import { apiRefreshToken, apiLogout } from "./auth";

export const AUTH_SESSION_REFRESHED_EVENT = "ignitify:auth-session-refreshed";
export const AUTH_SESSION_CLEARED_EVENT = "ignitify:auth-session-cleared";

let accessToken: string | null = null;
let refreshPromise: Promise<AuthSession | null> | null = null;

export function getToken(): string | null {
  return accessToken;
}

export function setToken(token: string): void {
  accessToken = token;
}

export function clearToken(): void {
  accessToken = null;
}

export function applyAuthSession(session: AuthSession): void {
  setToken(session.access_token);
  window.dispatchEvent(
    new CustomEvent<AuthSession>(AUTH_SESSION_REFRESHED_EVENT, { detail: session }),
  );
}

export function clearAuthTokens(): void {
  clearToken();
  window.dispatchEvent(new Event(AUTH_SESSION_CLEARED_EVENT));
}

export function beginLogout(): void {
  clearAuthTokens();
  void apiLogout();
}

export async function refreshStoredAccessToken(): Promise<AuthSession | null> {
  if (refreshPromise) return refreshPromise;
  refreshPromise = apiRefreshToken()
    .then((result) => {
      if (!result.success) {
        clearAuthTokens();
        return null;
      }
      applyAuthSession(result.data);
      return result.data;
    })
    .catch(() => null)
    .finally(() => {
      refreshPromise = null;
    });
  return refreshPromise;
}

export async function waitForServer(): Promise<void> {
  return Promise.resolve();
}

export function resetServerReadyGate(): void {
  // Backend readiness polling belongs to desktop/runtime integration layer.
}
