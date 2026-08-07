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
} {
  const domainBlock =
    application.templateToml?.match(
      /\[\[config\.domains\]\]([\s\S]*?)(?=\r?\n\[\[|\r?\n\[|$)/,
    )?.[1] ?? "";
  const exposedService =
    domainBlock.match(/^\s*serviceName\s*=\s*"([^"]+)"\s*$/m)?.[1] ??
    application.composeYaml.match(
      /^services:\s*\r?\n(?:[ \t]*#.*\r?\n)*[ \t]+([A-Za-z0-9][A-Za-z0-9._-]*):\s*(?:#.*)?$/m,
    )?.[1] ??
    "";
  const internalPort = domainBlock.match(/^\s*port\s*=\s*"?(\d+)"?\s*(?:#.*)?$/m)?.[1] ?? "";

  return { exposedService, internalPort };
}
