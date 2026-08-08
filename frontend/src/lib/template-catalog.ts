import { TEMPLATES_URL } from "./api/templates";
import type { TemplateLinks, TemplateSummary } from "./types";

export interface TemplateMetadata {
  id: string;
  name: string;
  version: string;
  description: string;
  logo: string;
  links: TemplateLinks;
  tags: string[];
}

export interface TemplateApplication {
  template: TemplateMetadata;
  composeYaml: string;
  templateToml: string | null;
}

export interface TemplateRuntimeVariable {
  key: string;
  value: string;
  is_secret: boolean;
}

interface TemplateDomainConfig {
  serviceName?: string;
  port?: string | number;
}

interface TemplateTomlConfig {
  variables: Record<string, string>;
  env: string[];
  domains: TemplateDomainConfig[];
}

interface ResolvedTemplateValue {
  value: string;
  isSecret: boolean;
}

export function toTemplateMetadata(template: TemplateSummary): TemplateMetadata {
  return {
    id: template.id,
    name: template.name,
    version: template.version ?? "latest",
    description: template.description ?? "No description provided.",
    logo: template.logo ?? "",
    links: template.links ?? {},
    tags: template.tags ?? [],
  };
}

export function templateFileUrl(templateId: string, filename: string): string {
  return `${TEMPLATES_URL.replace(/\/+$/, "")}/${encodeURIComponent(templateId)}/${encodeURIComponent(filename)}`;
}

export function templateRuntimeDefaults(application: TemplateApplication): {
  exposedService: string;
  internalPort: string;
  variables: TemplateRuntimeVariable[];
} {
  const config = parseTemplateToml(application.templateToml);
  const resolvedVariables = new Map<string, ResolvedTemplateValue>();
  const resolvingVariables = new Set<string>();
  const domain = config.domains.find((entry) => entry.serviceName) ?? config.domains[0];
  const exposedService =
    domain?.serviceName ??
    application.composeYaml.match(
      /^services:\s*\r?\n(?:[ \t]*#.*\r?\n)*[ \t]+([A-Za-z0-9][A-Za-z0-9._-]*):\s*(?:#.*)?$/m,
    )?.[1] ??
    "";
  const internalPort = domain?.port?.toString() ?? "";
  const variables = templateEnvironmentVariables(config, resolvedVariables, resolvingVariables);

  return { exposedService, internalPort, variables };
}

function parseTemplateToml(source: string | null): TemplateTomlConfig {
  const config: TemplateTomlConfig = { variables: {}, env: [], domains: [] };
  if (!source) return config;

  let section: "variables" | "config" | "domain" | null = null;
  let currentDomain: TemplateDomainConfig | null = null;
  const lines = source.replace(/\r\n/g, "\n").split("\n");

  for (let index = 0; index < lines.length; index += 1) {
    const statement = stripTomlComment(lines[index] ?? "").trim();
    if (!statement) continue;

    if (statement === "[variables]") {
      section = "variables";
      currentDomain = null;
      continue;
    }
    if (statement === "[config]") {
      section = "config";
      currentDomain = null;
      continue;
    }
    if (statement === "[[config.domains]]") {
      section = "domain";
      currentDomain = {};
      config.domains.push(currentDomain);
      continue;
    }
    if (statement.startsWith("[") && statement.endsWith("]")) {
      section = null;
      currentDomain = null;
      continue;
    }

    const equalsIndex = findTomlDelimiter(statement, "=");
    if (equalsIndex < 1) continue;

    const key = unquoteTomlKey(statement.slice(0, equalsIndex).trim());
    let value = statement.slice(equalsIndex + 1).trim();
    while (!isCompleteTomlValue(value) && index + 1 < lines.length) {
      index += 1;
      value += `\n${stripTomlComment(lines[index] ?? "").trim()}`;
    }
    const parsedValue = parseTomlValue(value);

    if (section === "variables" && typeof parsedValue === "string") {
      config.variables[key] = parsedValue;
      continue;
    }
    if (section === "config" && key === "env" && Array.isArray(parsedValue)) {
      config.env = parsedValue.filter((entry): entry is string => typeof entry === "string");
      continue;
    }
    if (
      section === "domain" &&
      currentDomain &&
      (key === "serviceName" || key === "port") &&
      (typeof parsedValue === "string" || typeof parsedValue === "number")
    ) {
      if (key === "serviceName") currentDomain.serviceName = parsedValue.toString();
      else currentDomain.port = parsedValue;
    }
  }

  return config;
}

function stripTomlComment(value: string) {
  return value.slice(0, findTomlDelimiter(value, "#"));
}

function findTomlDelimiter(value: string, delimiter: string) {
  let quote: '"' | "'" | null = null;
  let escaped = false;

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (quote === '"' && escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && character === "\\") {
      escaped = true;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = quote === character ? null : (quote ?? character);
      continue;
    }
    if (!quote && character === delimiter) return index;
  }

  return delimiter === "#" ? value.length : -1;
}

function isCompleteTomlValue(value: string) {
  let arrayDepth = 0;
  let quote: '"' | "'" | null = null;
  let escaped = false;

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (quote === '"' && escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && character === "\\") {
      escaped = true;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = quote === character ? null : (quote ?? character);
      continue;
    }
    if (quote) continue;
    if (character === "[") arrayDepth += 1;
    if (character === "]") arrayDepth -= 1;
  }

  return !quote && arrayDepth === 0;
}

function unquoteTomlKey(value: string) {
  const parsed = parseTomlString(value);
  return parsed ?? value;
}

function parseTomlValue(
  value: string,
): string | number | boolean | Array<string | number | boolean> | null {
  const parsedString = parseTomlString(value);
  if (parsedString !== null) return parsedString;
  if (/^-?\d+$/.test(value)) return Number(value);
  if (value === "true") return true;
  if (value === "false") return false;
  if (!value.startsWith("[") || !value.endsWith("]")) return null;

  const entries = splitTomlArray(value.slice(1, -1));
  const parsedEntries = entries.map((entry) => parseTomlValue(entry.trim()));
  return parsedEntries.every((entry) => entry !== null)
    ? (parsedEntries as Array<string | number | boolean>)
    : null;
}

function parseTomlString(value: string) {
  if (value.length < 2) return null;
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      return JSON.parse(value) as string;
    } catch {
      return null;
    }
  }
  if (value.startsWith("'") && value.endsWith("'")) return value.slice(1, -1);
  return null;
}

function splitTomlArray(value: string) {
  const entries: string[] = [];
  let start = 0;
  let quote: '"' | "'" | null = null;
  let escaped = false;

  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (quote === '"' && escaped) {
      escaped = false;
      continue;
    }
    if (quote === '"' && character === "\\") {
      escaped = true;
      continue;
    }
    if (character === '"' || character === "'") {
      quote = quote === character ? null : (quote ?? character);
      continue;
    }
    if (!quote && character === ",") {
      entries.push(value.slice(start, index));
      start = index + 1;
    }
  }
  entries.push(value.slice(start));
  return entries.filter((entry) => entry.trim());
}

function templateEnvironmentVariables(
  config: TemplateTomlConfig,
  resolvedVariables: Map<string, ResolvedTemplateValue>,
  resolvingVariables: Set<string>,
) {
  const variables = new Map<string, TemplateRuntimeVariable>();
  const source = config.env.length
    ? config.env
    : Object.entries(config.variables).map(([key, value]) => `${key}=${value}`);

  for (const entry of source) {
    const separator = entry.indexOf("=");
    const key = entry.slice(0, separator).trim();
    if (separator < 1 || !key) continue;

    const resolved = resolveTemplateValue(
      entry.slice(separator + 1),
      config.variables,
      resolvedVariables,
      resolvingVariables,
    );
    variables.set(key, { key, value: resolved.value, is_secret: resolved.isSecret });
  }

  return [...variables.values()];
}

function resolveTemplateValue(
  value: string,
  definitions: Record<string, string>,
  resolvedVariables: Map<string, ResolvedTemplateValue>,
  resolvingVariables: Set<string>,
): ResolvedTemplateValue {
  let isSecret = false;
  const resolved = value.replace(/\$\{([^}]+)\}/g, (_match, identifier: string) => {
    const passwordLength = /^password:(\d+)$/.exec(identifier);
    if (passwordLength) {
      isSecret = true;
      return generatePassword(Number(passwordLength[1]));
    }

    const variable = resolveTemplateVariable(
      identifier,
      definitions,
      resolvedVariables,
      resolvingVariables,
    );
    isSecret ||= variable.isSecret;
    return variable.value;
  });

  return { value: resolved, isSecret };
}

function resolveTemplateVariable(
  key: string,
  definitions: Record<string, string>,
  resolvedVariables: Map<string, ResolvedTemplateValue>,
  resolvingVariables: Set<string>,
): ResolvedTemplateValue {
  const existing = resolvedVariables.get(key);
  if (existing) return existing;
  const definition = definitions[key];
  if (definition === undefined || resolvingVariables.has(key)) {
    return { value: "", isSecret: false };
  }

  resolvingVariables.add(key);
  const resolved = resolveTemplateValue(
    definition,
    definitions,
    resolvedVariables,
    resolvingVariables,
  );
  resolvingVariables.delete(key);
  resolvedVariables.set(key, resolved);
  return resolved;
}

function generatePassword(length: number) {
  const size = Math.min(Math.max(length, 1), 4096);
  const crypto = globalThis.crypto;
  if (!crypto?.getRandomValues) return "";

  const values = new Uint8Array(size);
  crypto.getRandomValues(values);
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  return Array.from(values, (value) => alphabet[value & 63]).join("");
}
