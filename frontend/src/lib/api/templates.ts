import type { TemplateSummary } from "../types";
import type { ApiResult } from "./core";

export const DEFAULT_TEMPLATES_URL = "http://localhost:4545/api/templates";
export const TEMPLATES_URL = import.meta.env.VITE_TEMPLATES_URL?.trim() || DEFAULT_TEMPLATES_URL;

const TEMPLATES_TIMEOUT_MS = 30_000;

export interface TemplateCatalogQuery {
  page?: number;
  pageSize?: number;
  query?: string;
  tag?: string;
}

export interface TemplateCatalogPage {
  items: TemplateSummary[];
  page: number;
  pageSize: number;
  total: number;
  totalPages: number;
  hasNextPage: boolean;
  hasPreviousPage: boolean;
}

export type TemplateCatalogResponse = TemplateCatalogPage | TemplateSummary[];

export async function apiListTemplates(
  query: TemplateCatalogQuery = {},
): Promise<ApiResult<TemplateCatalogResponse>> {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), TEMPLATES_TIMEOUT_MS);

  const params = new URLSearchParams({
    page: String(Math.max(1, query.page ?? 1)),
    page_size: String(Math.min(100, Math.max(1, query.pageSize ?? 24))),
  });
  if (query.query?.trim()) params.set("q", query.query.trim());
  if (query.tag && query.tag !== "all") params.set("tag", query.tag);

  try {
    const response = await fetch(
      `${TEMPLATES_URL}${TEMPLATES_URL.includes("?") ? "&" : "?"}${params}`,
      {
        credentials: "omit",
        signal: controller.signal,
      },
    );

    if (!response.ok) {
      const errorText = await response.text();
      return {
        success: false,
        data: [],
        error: errorText || `Templates API returned ${response.status}`,
        status: response.status,
      };
    }

    const payload = (await response.json()) as unknown;
    if (Array.isArray(payload)) {
      return { success: true, data: payload as TemplateSummary[] };
    }

    if (
      !payload ||
      typeof payload !== "object" ||
      !Array.isArray((payload as { items?: unknown }).items)
    ) {
      return {
        success: false,
        data: [],
        error: "Templates API returned an invalid catalog.",
      };
    }

    return { success: true, data: payload as TemplateCatalogPage };
  } catch (cause) {
    return {
      success: false,
      data: [],
      error:
        cause instanceof Error && cause.name === "AbortError"
          ? "Loading templates timed out."
          : cause instanceof Error
            ? cause.message
            : "Could not load templates.",
    };
  } finally {
    clearTimeout(timeoutId);
  }
}
