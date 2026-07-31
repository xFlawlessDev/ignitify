import { API_BASE } from "../constants";
import { getToken, refreshStoredAccessToken, resetServerReadyGate, waitForServer } from "./session";

export {
  AUTH_SESSION_CLEARED_EVENT,
  AUTH_SESSION_REFRESHED_EVENT,
  applyAuthSession,
  beginLogout,
  clearAuthTokens,
  clearToken,
  getToken,
  refreshStoredAccessToken,
  setToken,
  waitForServer,
} from "./session";

// ---------------------------------------------------------------------------
// Generic fetch wrapper
// ---------------------------------------------------------------------------

export interface ApiResult<T> {
  success: boolean;
  data: T;
  error?: string;
  errorCode?: string;
  status?: number;
}

const DEFAULT_TIMEOUT_MS = 30_000;
const VISION_CONFIG_TIMEOUT_MS = 130_000;
const MEDIA_GENERATION_TIMEOUT_MS = 300_000;

export function apiFetchTimeoutMs(endpoint: string): number {
  if (endpoint === "/providers/vision") return VISION_CONFIG_TIMEOUT_MS;
  if (endpoint.startsWith("/media/admin/test/")) return MEDIA_GENERATION_TIMEOUT_MS;
  return DEFAULT_TIMEOUT_MS;
}

export function apiFetchTimeoutMessage(endpoint: string): string {
  if (endpoint === "/providers/vision") {
    return `Saving vision config timed out after ${VISION_CONFIG_TIMEOUT_MS / 1000}s. Check backend logs/model files and try again.`;
  }
  if (endpoint.startsWith("/media/admin/test/")) {
    return `Media pipeline test timed out after ${MEDIA_GENERATION_TIMEOUT_MS / 1000}s. Check provider/backend logs, then refresh media jobs.`;
  }
  return "Request timed out";
}

export async function apiFetch<T>(
  endpoint: string,
  options: RequestInit = {},
): Promise<ApiResult<T>> {
  await waitForServer();

  const headers = new Headers(options.headers);
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), apiFetchTimeoutMs(endpoint));

  let response: Response;
  try {
    response = await apiFetchRaw(endpoint, {
      ...options,
      headers,
      signal: options.signal ?? controller.signal,
    });
  } catch (error) {
    clearTimeout(timeoutId);
    if (error instanceof DOMException && error.name === "AbortError") {
      return {
        success: false,
        data: null as unknown as T,
        error: apiFetchTimeoutMessage(endpoint),
      };
    }
    // Network-level failure (NS_CONNECTION_REFUSED, ECONNRESET, etc.) means
    // the backend is down. Reset the server-ready gate so the next apiFetch()
    // re-polls instead of skipping straight to a dead endpoint.
    if (error instanceof TypeError) {
      resetServerReadyGate();
    }
    return {
      success: false,
      data: null as unknown as T,
      error: error instanceof Error ? error.message : "Network request failed",
    };
  }
  clearTimeout(timeoutId);

  if (!response.ok) {
    const errorText = await response.text();
    let errorMsg = `API Error: ${response.status} ${response.statusText}`;
    let errorCode: string | undefined;
    try {
      const errorJson = JSON.parse(errorText);
      errorMsg = errorJson.error?.message || errorJson.error || errorJson.message || errorMsg;
      errorCode = errorJson.error?.code || errorJson.code;
    } catch {
      if (errorText) errorMsg = errorText;
    }
    return {
      success: false,
      data: null as unknown as T,
      error: errorMsg,
      errorCode,
      status: response.status,
    };
  }

  if (response.status === 204) {
    return { success: true, data: null as unknown as T };
  }

  const body = await response.json();
  return { success: true, data: body };
}

export async function apiFetchRaw(
  endpointOrUrl: string,
  options: RequestInit = {},
): Promise<Response> {
  await waitForServer();
  const url = resolveApiUrl(endpointOrUrl);
  return fetchWithAuth(url, { credentials: "same-origin", ...options });
}

export async function apiFetchBlob(
  endpoint: string,
  options: RequestInit = {},
): Promise<ApiResult<Blob>> {
  await waitForServer();

  const response = await apiFetchRaw(endpoint, {
    ...options,
  });

  if (!response.ok) {
    const errorText = await response.text();
    let errorMsg = `API Error: ${response.status} ${response.statusText}`;
    try {
      const errorJson = JSON.parse(errorText);
      errorMsg = errorJson.error?.message || errorJson.error || errorJson.message || errorMsg;
    } catch {
      if (errorText) errorMsg = errorText;
    }
    return { success: false, data: new Blob(), error: errorMsg };
  }

  return { success: true, data: await response.blob() };
}

async function fetchWithAuth(
  url: string,
  options: RequestInit,
  canRetry = true,
): Promise<Response> {
  const response = await fetch(url, withAuthorization(url, options));
  if (response.status !== 401 || !canRetry || !shouldRefreshFor(url)) {
    return response;
  }

  const refreshed = await refreshStoredAccessToken();
  if (!refreshed) {
    return response;
  }

  return fetch(url, withAuthorization(url, options));
}

function withAuthorization(url: string, options: RequestInit): RequestInit {
  const headers = new Headers(options.headers);
  if ((options.method ?? "GET").toUpperCase() !== "GET" && isSameOrigin(url)) {
    headers.set("X-Ignitify-Request", "1");
  }
  const token = getToken();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  } else {
    headers.delete("Authorization");
  }
  return { credentials: "same-origin", ...options, headers };
}

function resolveApiUrl(endpointOrUrl: string): string {
  if (/^https?:\/\//i.test(endpointOrUrl)) {
    return endpointOrUrl;
  }
  if (endpointOrUrl.startsWith(API_BASE)) {
    return endpointOrUrl;
  }
  return `${API_BASE}${endpointOrUrl.startsWith("/") ? endpointOrUrl : `/${endpointOrUrl}`}`;
}

function shouldRefreshFor(url: string): boolean {
  const path = extractPath(url);
  if (!path.startsWith(API_BASE)) return false;
  const endpoint = path.slice(API_BASE.length) || "/";
  return !(
    endpoint === "/auth/bootstrap" ||
    endpoint === "/auth/login" ||
    endpoint === "/auth/refresh" ||
    endpoint === "/auth/logout" ||
    endpoint.startsWith("/auth/password/")
  );
}

function isSameOrigin(url: string): boolean {
  const base = typeof window === "undefined" ? "http://localhost" : window.location.origin;
  try {
    return new URL(url, base).origin === new URL(base).origin;
  } catch {
    return false;
  }
}

function extractPath(url: string): string {
  try {
    const base = typeof window === "undefined" ? "http://localhost" : window.location.origin;
    return new URL(url, base).pathname;
  } catch {
    return url;
  }
}
