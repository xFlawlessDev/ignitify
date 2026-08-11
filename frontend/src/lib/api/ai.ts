import type { ApiResult } from "./core";
import { apiFetch } from "./core";

export interface AiSettings {
  enabled: boolean;
  base_url: string;
  model: string;
  api_key_configured: boolean;
  created_at: string;
  updated_at: string;
}

export interface AiSettingsInput {
  enabled: boolean;
  base_url: string;
  model: string;
  api_key?: string;
  clear_api_key: boolean;
}

export interface AiChatMessage {
  role: "user" | "assistant";
  content: string;
}

export interface AiLogContext {
  label: string;
  content: string;
}

export interface AiChatInput {
  messages: AiChatMessage[];
  log_context?: AiLogContext;
}

export interface AiChatResponse {
  content: string;
}

const settingsEndpoint = "/settings/ai";

export function apiGetAiSettings(): Promise<ApiResult<AiSettings>> {
  return apiFetch<AiSettings>(settingsEndpoint);
}

export function apiUpdateAiSettings(input: AiSettingsInput): Promise<ApiResult<AiSettings>> {
  return apiFetch<AiSettings>(settingsEndpoint, {
    method: "PUT",
    body: JSON.stringify(input),
  });
}

export function apiChatWithAi(input: AiChatInput): Promise<ApiResult<AiChatResponse>> {
  return apiFetch<AiChatResponse>("/ai/chat", {
    method: "POST",
    body: JSON.stringify(input),
  });
}
