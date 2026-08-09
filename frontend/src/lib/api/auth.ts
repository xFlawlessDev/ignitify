import type {
  AuthSession,
  MessageResponse,
  AuthenticatedUser,
  StepUpSession,
} from "../types";

import { apiFetch } from "./core";
import type { ApiResult } from "./core";

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

export interface BootstrapStatus {
  required: boolean;
  enabled: boolean;
}

export async function apiBootstrapStatus(): Promise<ApiResult<BootstrapStatus>> {
  return apiFetch<BootstrapStatus>("/auth/bootstrap");
}

export async function apiBootstrap(
  username: string,
  password: string,
  bootstrapSecret: string,
): Promise<ApiResult<AuthSession>> {
  return apiFetch<AuthSession>("/auth/bootstrap", {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "X-Ignitify-Request": "1",
      "X-Ignitify-Bootstrap-Secret": bootstrapSecret,
    },
    body: JSON.stringify({ username, password }),
  });
}

export async function apiLogin(
  username: string,
  password: string,
): Promise<ApiResult<AuthSession>> {
  return apiFetch<AuthSession>("/auth/login", {
    method: "POST",
    credentials: "same-origin",
    headers: { "X-Ignitify-Request": "1" },
    body: JSON.stringify({ username, password }),
  });
}

export async function apiStepUp(password: string): Promise<ApiResult<StepUpSession>> {
  return apiFetch<StepUpSession>("/auth/step-up", {
    method: "POST",
    credentials: "same-origin",
    headers: { "X-Ignitify-Request": "1" },
    body: JSON.stringify({ password }),
  });
}

export async function apiRefreshToken(): Promise<ApiResult<AuthSession>> {
  return apiFetch<AuthSession>("/auth/refresh", {
    method: "POST",
    credentials: "same-origin",
    headers: { "X-Ignitify-Request": "1" },
  });
}

export async function apiLogout(): Promise<ApiResult<MessageResponse>> {
  return apiFetch<MessageResponse>("/auth/logout", {
    method: "POST",
    credentials: "same-origin",
    headers: { "X-Ignitify-Request": "1" },
  });
}

export async function apiGetMe(): Promise<ApiResult<AuthenticatedUser>> {
  return apiFetch<AuthenticatedUser>("/auth/me");
}
