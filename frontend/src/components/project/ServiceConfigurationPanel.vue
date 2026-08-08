<script setup lang="ts">
import { Box, Boxes, CircleAlert, FileCode2, GitBranch, Info, Plus, Trash2 } from "@lucide/vue";
import { computed, reactive, shallowRef, watch } from "vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import TemplateCatalogPicker from "@/components/templates/TemplateCatalogPicker.vue";
import YamlCodeEditor from "@/components/project/YamlCodeEditor.vue";
import { useProviderRepositories } from "@/composables/useProviderRepositories";
import { templateRuntimeDefaults, type TemplateApplication } from "@/lib/template-catalog";
import { cn } from "@/lib/utils";
import type {
  ApplicationBuilder,
  ProjectEnvironmentVariable,
  ProviderSummary,
  ServiceInput,
  ServiceSource,
  ServiceSummary,
  ServiceVariable,
} from "@/lib/types";

const props = defineProps<{
  error?: string | null;
  inheritedVariables?: ProjectEnvironmentVariable[];
  providers?: ProviderSummary[];
  saving?: boolean;
  service: ServiceSummary;
}>();

const emit = defineEmits<{ save: [input: ServiceInput] }>();

const name = shallowRef("");
const kind = shallowRef<"image" | "compose">("image");
const source = shallowRef<ServiceSource>("template");
const composeMode = shallowRef<"yaml" | "repository">("yaml");
const template = shallowRef("static");
const providerId = shallowRef("");
const repository = shallowRef("");
const branch = shallowRef("main");
const builder = shallowRef<ApplicationBuilder>("static");
const dockerfilePath = shallowRef("Dockerfile");
const buildCommand = shallowRef("");
const outputDirectory = shallowRef("dist");
const imageReference = shallowRef("");
const composeYaml = shallowRef("");
const exposedService = shallowRef("");
const internalPort = shallowRef("");
const healthcheck = shallowRef("");
const appliedTemplateName = shallowRef("");
const validationError = shallowRef<string | null>(null);
const variables = reactive<ServiceVariable[]>([]);
const sourceRepositories = useProviderRepositories();

const builderOptions: Array<{ value: ApplicationBuilder; label: string; description: string }> = [
  { value: "static", label: "Static", description: "Build and serve static assets" },
  { value: "dockerfile", label: "Dockerfile", description: "Use the repository Dockerfile" },
  { value: "railpack", label: "Railpack", description: "Detect and build the app" },
];
const sourceOptions = [
  {
    value: "template" as const,
    label: "Template",
    icon: Box,
    description: "Catalog runtime",
  },
  {
    value: "compose" as const,
    label: "Compose",
    icon: FileCode2,
    description: "Managed YAML",
  },
  {
    value: "application" as const,
    label: "Application",
    icon: GitBranch,
    description: "Git build",
  },
];
const availableProviders = computed(() =>
  (props.providers ?? []).filter((provider) => provider.token_configured),
);
const repositoryOptions = computed(() => {
  const current = repository.value;
  if (current && !sourceRepositories.repositories.value.some((item) => item.path === current)) {
    return [
      { name: current, path: current, default_branch: branch.value || null },
      ...sourceRepositories.repositories.value,
    ];
  }
  return sourceRepositories.repositories.value;
});
const branchOptions = computed(() => {
  const current = branch.value;
  if (current && !sourceRepositories.branches.value.some((item) => item.name === current)) {
    return [{ name: current }, ...sourceRepositories.branches.value];
  }
  return sourceRepositories.branches.value;
});
const sourceSummary = computed(() => {
  if (source.value === "application") {
    return repository.value.trim() || `${builder.value} build`;
  }
  if (source.value === "compose") {
    return composeMode.value === "repository"
      ? repository.value.trim() || "Repository Compose"
      : "Inline Compose";
  }
  return appliedTemplateName.value || "Template runtime";
});
const validationMessage = computed(() => validationError.value ?? props.error ?? null);
const supportsBuildCommand = computed(
  () =>
    source.value === "application" && (builder.value === "static" || builder.value === "railpack"),
);
const usesStaticOutput = computed(
  () => source.value === "application" && builder.value === "static",
);

function isRepositorySource() {
  return (
    source.value === "application" ||
    (source.value === "compose" && composeMode.value === "repository")
  );
}

function sourceOptionClass(selected: boolean) {
  return cn(
    "grid gap-1 border px-3 py-3 text-left transition-colors",
    selected
      ? "border-[var(--status-live)] bg-muted"
      : "border-border hover:bg-muted focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
  );
}

function builderOptionClass(selected: boolean) {
  return cn(
    "grid gap-1 border px-3 py-2 text-left transition-colors",
    selected
      ? "border-[var(--status-live)] bg-background"
      : "border-border hover:bg-background focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
  );
}

function composeModeClass(selected: boolean) {
  return cn(
    "border px-3 py-2 text-left text-xs transition-colors",
    selected
      ? "border-[var(--status-live)] bg-muted"
      : "border-border hover:bg-muted focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
  );
}

function selectProvider(value: string) {
  providerId.value = value;
  repository.value = "";
  branch.value = "";
  void sourceRepositories.loadRepositories(value);
}

function selectRepository(value: string) {
  repository.value = value;
  const selected = sourceRepositories.repositories.value.find((item) => item.path === value);
  branch.value = selected?.default_branch ?? "";
  void sourceRepositories.loadBranches(providerId.value, value);
}

function selectSource(value: ServiceSource) {
  const previousSource = source.value;
  source.value = value;
  kind.value = value === "compose" ? "compose" : "image";
  validationError.value = null;
  if (value === "template" && previousSource !== "template") {
    composeYaml.value = "";
    exposedService.value = "";
    internalPort.value = "";
    appliedTemplateName.value = "";
  }
  if (value !== "template") {
    appliedTemplateName.value = "";
  }
  if (value === "compose" && dockerfilePath.value === "Dockerfile") {
    dockerfilePath.value = "docker-compose.yml";
  }
  if (value === "application" && builder.value === "static") internalPort.value = "80";
  if (!isRepositorySource()) {
    sourceRepositories.reset();
  }
}

function selectComposeMode(value: "yaml" | "repository") {
  composeMode.value = value;
  validationError.value = null;
  if (value === "repository" && dockerfilePath.value === "Dockerfile") {
    dockerfilePath.value = "docker-compose.yml";
  }
  if (value === "repository" && providerId.value) {
    void sourceRepositories.loadRepositories(providerId.value);
    return;
  }
  if (value === "yaml") sourceRepositories.reset();
}

function selectProviderEvent(event: Event) {
  selectProvider((event.target as HTMLSelectElement).value);
}

function selectRepositoryEvent(event: Event) {
  selectRepository((event.target as HTMLSelectElement).value);
}

function reset() {
  sourceRepositories.reset();
  name.value = props.service.name;
  kind.value = props.service.kind;
  source.value =
    props.service.source_config?.source ?? (kind.value === "compose" ? "compose" : "template");
  composeMode.value = props.service.source_config?.provider_id ? "repository" : "yaml";
  template.value = props.service.source_config?.setup_required
    ? "static"
    : (props.service.source_config?.template ?? "static");
  appliedTemplateName.value = props.service.source_config?.template ?? "";
  providerId.value = props.service.source_config?.provider_id ?? "";
  repository.value = props.service.source_config?.repository ?? "";
  branch.value = props.service.source_config?.branch ?? "main";
  builder.value =
    props.service.source_config?.builder === "spa"
      ? "static"
      : (props.service.source_config?.builder ?? "static");
  dockerfilePath.value =
    props.service.source_config?.dockerfile_path ??
    (props.service.source_config?.source === "compose" ? "docker-compose.yml" : "Dockerfile");
  buildCommand.value = props.service.source_config?.build_command ?? "";
  outputDirectory.value = props.service.source_config?.output_directory ?? "dist";
  imageReference.value = props.service.image_reference ?? "";
  composeYaml.value = props.service.compose_yaml ?? "";
  exposedService.value = props.service.exposed_service ?? "";
  internalPort.value =
    source.value === "application" && builder.value === "static"
      ? "80"
      : (props.service.internal_port?.toString() ?? "");
  healthcheck.value = props.service.healthcheck?.join("\n") ?? "";
  validationError.value = null;
  variables.splice(
    0,
    variables.length,
    ...props.service.variables.map((variable) => ({
      key: variable.key,
      value: variable.is_secret ? "" : (variable.value ?? ""),
      is_secret: variable.is_secret,
    })),
  );
  if (providerId.value && isRepositorySource()) {
    void sourceRepositories.loadRepositories(providerId.value).then(() => {
      if (repository.value)
        void sourceRepositories.loadBranches(providerId.value, repository.value);
    });
  }
}

function addVariable() {
  variables.push({ key: "", value: "", is_secret: true });
}

function selectBuilder(value: ApplicationBuilder) {
  builder.value = value;
  if (value === "static") internalPort.value = "80";
}

function removeVariable(index: number) {
  variables.splice(index, 1);
}

function normalizeComposeYaml(value: string) {
  return value.replace(/\r\n/g, "\n").split(String.fromCharCode(0)).join("");
}

function applyTemplate(application: TemplateApplication) {
  const defaults = templateRuntimeDefaults(application);
  source.value = "template";
  template.value = application.template.id;
  appliedTemplateName.value = application.template.name;
  kind.value = "compose";
  composeMode.value = "yaml";
  composeYaml.value = normalizeComposeYaml(application.composeYaml);
  exposedService.value = defaults.exposedService;
  internalPort.value = defaults.internalPort;
  variables.splice(0, variables.length, ...defaults.variables);
  validationError.value = null;
}

function isDigestPinnedImage(value: string) {
  return /^[^\s@]+@sha256:[a-fA-F0-9]{64}$/.test(value);
}

function submit() {
  validationError.value = null;
  const submittedComposeYaml = normalizeComposeYaml(composeYaml.value);
  composeYaml.value = submittedComposeYaml;
  if (source.value === "template" && !submittedComposeYaml.trim()) {
    validationError.value = "Choose a template before saving changes.";
    return;
  }
  if (
    source.value !== "application" &&
    source.value !== "template" &&
    kind.value === "image" &&
    !isDigestPinnedImage(imageReference.value.trim())
  ) {
    validationError.value = "Image reference must include an exact sha256 digest.";
    return;
  }
  if (
    kind.value === "compose" &&
    (!exposedService.value.trim() || (composeMode.value === "yaml" && !submittedComposeYaml.trim()))
  ) {
    validationError.value = "Compose YAML and exposed service are required.";
    return;
  }
  if (
    source.value === "compose" &&
    composeMode.value === "repository" &&
    (!providerId.value || !repository.value.trim() || !branch.value.trim())
  ) {
    validationError.value = "Choose a provider, repository, and branch for the Compose file.";
    return;
  }
  if (variables.some((variable) => variable.is_secret && !variable.value)) {
    validationError.value = "Re-enter each secret value before saving changes.";
    return;
  }
  if (
    source.value === "application" &&
    (!providerId.value || !repository.value.trim() || !branch.value.trim())
  ) {
    validationError.value = "Choose a provider, repository, and branch for the application.";
    return;
  }
  const port = String(internalPort.value).trim();
  const parsedPort = port ? Number(port) : null;
  if (
    parsedPort !== null &&
    (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535)
  ) {
    validationError.value = "Internal port must be between 1 and 65535.";
    return;
  }
  const healthcheckArguments = healthcheck.value
    .split("\n")
    .map((argument) => argument.trim())
    .filter(Boolean);
  emit("save", {
    name: name.value.trim(),
    kind: kind.value,
    ...(kind.value === "image"
      ? {
          image_reference: imageReference.value.trim(),
          healthcheck: healthcheckArguments.length ? healthcheckArguments : null,
        }
      : {
          compose_yaml: submittedComposeYaml,
          exposed_service: exposedService.value.trim(),
          healthcheck: null,
        }),
    internal_port: parsedPort,
    variables: variables.map((variable) => ({ ...variable, key: variable.key.trim() })),
    source_config: {
      source: source.value,
      ...(source.value === "template" ? { template: template.value } : {}),
      ...(source.value === "application"
        ? {
            provider_id: providerId.value,
            repository: repository.value.trim(),
            branch: branch.value.trim(),
            builder: builder.value,
            ...(builder.value === "dockerfile" && dockerfilePath.value.trim()
              ? { dockerfile_path: dockerfilePath.value.trim() }
              : {}),
            ...(supportsBuildCommand.value && buildCommand.value.trim()
              ? { build_command: buildCommand.value.trim() }
              : {}),
            ...(usesStaticOutput.value && outputDirectory.value.trim()
              ? { output_directory: outputDirectory.value.trim() }
              : {}),
          }
        : {}),
      ...(source.value === "compose" && composeMode.value === "repository"
        ? {
            provider_id: providerId.value,
            repository: repository.value.trim(),
            branch: branch.value.trim(),
            ...(dockerfilePath.value.trim()
              ? { dockerfile_path: dockerfilePath.value.trim() }
              : {}),
          }
        : {}),
    },
  });
}

watch(() => props.service.id, reset, { immediate: true });
</script>

<template>
  <form class="grid gap-6 border border-border bg-card p-5" @submit.prevent="submit">
    <header
      class="flex items-start justify-between gap-4 border-b border-border pb-4 max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Service configuration</p>
        <h2 class="mt-2 text-xl font-normal">Deployment source and runtime</h2>
      </div>
      <Badge variant="outline" class="font-mono text-[11px] text-muted-foreground">
        {{ sourceSummary }}
      </Badge>
    </header>

    <div class="grid gap-2">
      <Label for="service-config-name">Service name</Label>
      <Input id="service-config-name" v-model="name" maxlength="64" required />
    </div>

    <section class="grid gap-3 border-t border-border pt-4">
      <div>
        <p class="text-sm font-medium">Deployment source</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Choose how this service produces its runtime.
        </p>
      </div>
      <div class="grid gap-2 sm:grid-cols-3" role="group" aria-label="Deployment source">
        <button
          v-for="option in sourceOptions"
          :key="option.value"
          :class="sourceOptionClass(source === option.value)"
          type="button"
          :aria-pressed="source === option.value"
          @click="selectSource(option.value as ServiceSource)"
        >
          <component :is="option.icon" class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <span class="text-xs font-medium">{{ option.label }}</span>
          <span class="text-[11px] text-muted-foreground">{{ option.description }}</span>
        </button>
      </div>
    </section>

    <section v-if="source === 'template'" class="grid gap-3 border border-border bg-muted/30 p-3">
      <div class="flex items-start gap-2">
        <Boxes class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div>
          <p class="text-xs font-medium">Choose a template</p>
          <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
            Templates provide a starting runtime shape. You can still adjust the image and port.
          </p>
        </div>
      </div>
      <TemplateCatalogPicker @apply="applyTemplate" />
      <Badge v-if="appliedTemplateName" variant="outline" class="font-mono text-[11px]">
        {{ appliedTemplateName }}
      </Badge>
    </section>

    <section
      v-else-if="source === 'application'"
      class="grid gap-4 border border-border bg-muted/30 p-3"
    >
      <div class="flex items-start gap-2">
        <GitBranch class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div>
          <p class="text-xs font-medium">Repository build</p>
          <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
            Provider credentials stay server-side. The builder produces an immutable image for
            deployment.
          </p>
        </div>
      </div>
      <div class="grid gap-2">
        <Label for="service-config-provider">Repository provider</Label>
        <select
          id="service-config-provider"
          :value="providerId"
          class="h-9 border border-input bg-background px-3 text-sm"
          required
          @change="selectProviderEvent"
        >
          <option value="" disabled>Select a connected provider</option>
          <option v-for="provider in availableProviders" :key="provider.id" :value="provider.id">
            {{ provider.name }} ({{ provider.kind }})
          </option>
        </select>
        <p v-if="!availableProviders.length" class="text-[11px] text-muted-foreground">
          Connect a provider first in Providers.
        </p>
      </div>
      <div class="grid gap-3 sm:grid-cols-2">
        <label for="service-config-repository" class="grid gap-2 text-xs text-muted-foreground">
          Repository
          <select
            id="service-config-repository"
            :value="repository"
            class="h-9 border border-input bg-background px-3 text-sm text-foreground"
            :disabled="!providerId || sourceRepositories.repositoriesLoading.value"
            required
            @change="selectRepositoryEvent"
          >
            <option value="" disabled>
              {{
                sourceRepositories.repositoriesLoading.value
                  ? "Loading repositories..."
                  : "Select a repository"
              }}
            </option>
            <option v-for="option in repositoryOptions" :key="option.path" :value="option.path">
              {{ option.path }}
            </option>
          </select>
        </label>
        <label for="service-config-branch" class="grid gap-2 text-xs text-muted-foreground">
          Branch
          <select
            id="service-config-branch"
            v-model="branch"
            class="h-9 border border-input bg-background px-3 text-sm text-foreground"
            :disabled="!repository || sourceRepositories.branchesLoading.value"
            required
          >
            <option value="" disabled>
              {{
                sourceRepositories.branchesLoading.value ? "Loading branches..." : "Select a branch"
              }}
            </option>
            <option v-for="option in branchOptions" :key="option.name" :value="option.name">
              {{ option.name }}
            </option>
          </select>
        </label>
      </div>
      <p v-if="sourceRepositories.repositoriesError" class="text-xs text-destructive" role="alert">
        {{ sourceRepositories.repositoriesError }}
      </p>
      <p v-else-if="sourceRepositories.branchesError" class="text-xs text-destructive" role="alert">
        {{ sourceRepositories.branchesError }}
      </p>
      <div class="grid gap-2">
        <Label>Builder</Label>
        <div class="grid gap-2 sm:grid-cols-2">
          <button
            v-for="option in builderOptions"
            :key="option.value"
            :class="builderOptionClass(builder === option.value)"
            type="button"
            :aria-pressed="builder === option.value"
            @click="selectBuilder(option.value)"
          >
            <span class="text-xs font-medium">{{ option.label }}</span>
            <span class="text-[11px] leading-4 text-muted-foreground">{{
              option.description
            }}</span>
          </button>
        </div>
      </div>
      <div v-if="builder === 'dockerfile'" class="grid gap-2">
        <Label for="service-config-dockerfile">Dockerfile path</Label>
        <Input id="service-config-dockerfile" v-model="dockerfilePath" placeholder="Dockerfile" />
      </div>
      <div
        v-if="supportsBuildCommand"
        :class="cn('grid gap-3', usesStaticOutput && 'sm:grid-cols-2')"
      >
        <label for="service-config-build-command" class="grid gap-2 text-xs text-muted-foreground">
          Build command
          <Input
            id="service-config-build-command"
            v-model="buildCommand"
            placeholder="pnpm build"
          />
        </label>
        <label
          v-if="usesStaticOutput"
          for="service-config-output-directory"
          class="grid gap-2 text-xs text-muted-foreground"
        >
          Output directory
          <Input
            id="service-config-output-directory"
            v-model="outputDirectory"
            placeholder="dist"
          />
        </label>
      </div>
    </section>

    <template v-if="kind === 'image'">
      <div class="grid gap-2">
        <Label for="service-config-image">Runtime image digest</Label>
        <Input
          id="service-config-image"
          v-model="imageReference"
          placeholder="registry.example/app@sha256:..."
          required
        />
      </div>
    </template>
    <template v-else>
      <div v-if="source !== 'template'" class="grid gap-2">
        <Label>Compose source</Label>
        <div class="grid gap-2 sm:grid-cols-2">
          <button
            :class="composeModeClass(composeMode === 'yaml')"
            type="button"
            :aria-pressed="composeMode === 'yaml'"
            @click="selectComposeMode('yaml')"
          >
            Inline YAML
          </button>
          <button
            :class="composeModeClass(composeMode === 'repository')"
            type="button"
            :aria-pressed="composeMode === 'repository'"
            @click="selectComposeMode('repository')"
          >
            Provider repository
          </button>
        </div>
      </div>
      <div
        v-if="composeMode === 'repository'"
        class="grid gap-3 border border-border bg-muted/30 p-3 sm:grid-cols-3"
      >
        <label
          class="grid gap-2 text-xs text-muted-foreground"
          for="service-config-compose-provider"
        >
          Provider
          <select
            id="service-config-compose-provider"
            :value="providerId"
            class="h-9 border border-input bg-background px-3 text-sm text-foreground"
            required
            @change="selectProviderEvent"
          >
            <option value="" disabled>Select provider</option>
            <option v-for="provider in availableProviders" :key="provider.id" :value="provider.id">
              {{ provider.name }}
            </option>
          </select>
        </label>
        <label
          class="grid gap-2 text-xs text-muted-foreground"
          for="service-config-compose-repository"
        >
          Repository
          <select
            id="service-config-compose-repository"
            :value="repository"
            class="h-9 border border-input bg-background px-3 text-sm text-foreground"
            :disabled="!providerId || sourceRepositories.repositoriesLoading.value"
            required
            @change="selectRepositoryEvent"
          >
            <option value="" disabled>
              {{
                sourceRepositories.repositoriesLoading.value
                  ? "Loading repositories..."
                  : "Select a repository"
              }}
            </option>
            <option v-for="option in repositoryOptions" :key="option.path" :value="option.path">
              {{ option.path }}
            </option>
          </select>
        </label>
        <label class="grid gap-2 text-xs text-muted-foreground" for="service-config-compose-branch">
          Branch
          <select
            id="service-config-compose-branch"
            v-model="branch"
            class="h-9 border border-input bg-background px-3 text-sm text-foreground"
            :disabled="!repository || sourceRepositories.branchesLoading.value"
            required
          >
            <option value="" disabled>
              {{
                sourceRepositories.branchesLoading.value ? "Loading branches..." : "Select a branch"
              }}
            </option>
            <option v-for="option in branchOptions" :key="option.name" :value="option.name">
              {{ option.name }}
            </option>
          </select>
        </label>
        <label
          class="grid gap-2 text-xs text-muted-foreground sm:col-span-3"
          for="service-config-compose-path"
        >
          Compose file path
          <Input
            id="service-config-compose-path"
            v-model="dockerfilePath"
            placeholder="docker-compose.yml"
          />
        </label>
      </div>
      <p
        v-if="sourceRepositories.repositoriesError"
        class="text-[11px] text-destructive"
        role="alert"
      >
        {{ sourceRepositories.repositoriesError }}
      </p>
      <p
        v-else-if="sourceRepositories.branchesError"
        class="text-[11px] text-destructive"
        role="alert"
      >
        {{ sourceRepositories.branchesError }}
      </p>
      <div v-if="composeMode === 'yaml'" class="grid gap-2">
        <div class="flex items-center justify-between gap-3">
          <Label for="service-config-compose-yaml">
            {{ source === "template" ? "Template Compose YAML" : "Compose / Docker file" }}
          </Label>
          <span class="font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground">
            YAML
          </span>
        </div>
        <div class="overflow-hidden border border-border bg-background">
          <div
            class="flex items-center gap-2 border-b border-border bg-muted/40 px-4 py-2 font-mono text-[10px] uppercase tracking-[0.08em] text-muted-foreground"
          >
            <FileCode2 class="size-3.5 text-signal" :stroke-width="1.5" aria-hidden="true" />
            docker-compose.yml
          </div>
          <YamlCodeEditor
            id="service-config-compose-yaml"
            v-model="composeYaml"
            aria-label="Template Compose YAML"
            placeholder="services:\n  web:\n    image: registry.example/app:1.2.3"
            required
          />
        </div>
        <p class="text-xs text-muted-foreground">
          Every image must use an explicit tag or SHA-256 digest. No builds, host ports, binds,
          privileged mode, devices, or raw Traefik labels.
        </p>
      </div>
      <div class="grid gap-2">
        <Label for="service-config-exposed">Exposed Compose service</Label>
        <Input id="service-config-exposed" v-model="exposedService" placeholder="web" required />
      </div>
    </template>

    <div class="grid gap-2">
      <Label for="service-config-port">Internal port</Label>
      <Input id="service-config-port" v-model="internalPort" type="number" min="1" max="65535" />
    </div>
    <div class="grid gap-2">
      <Label for="service-config-healthcheck">Healthcheck argv</Label>
      <Textarea
        id="service-config-healthcheck"
        v-model="healthcheck"
        placeholder="One argument per line"
        spellcheck="false"
      />
    </div>

    <section v-if="inheritedVariables?.length" class="grid gap-2 border-t border-border pt-4">
      <div class="flex items-center gap-2">
        <Info class="size-3.5 text-muted-foreground" :stroke-width="1.5" />
        <h3 class="text-sm font-medium">Project defaults</h3>
      </div>
      <p class="text-xs leading-5 text-muted-foreground">
        These keys are inherited at deployment. Add a matching key below to override it for this
        service.
      </p>
      <div class="flex flex-wrap gap-1.5">
        <Badge
          v-for="variable in inheritedVariables"
          :key="variable.key"
          variant="outline"
          class="font-mono text-[10px] text-muted-foreground"
        >
          {{ variable.key }}
        </Badge>
      </div>
    </section>

    <fieldset class="grid gap-3 border-t border-border pt-4">
      <legend class="text-sm font-medium">Service environment</legend>
      <p class="-mt-1 text-xs leading-5 text-muted-foreground">
        Service keys override project defaults during deployment.
      </p>
      <div
        v-for="(variable, index) in variables"
        :key="index"
        class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-end gap-2 max-[480px]:grid-cols-1 max-[480px]:items-stretch"
      >
        <label class="grid gap-2 text-xs text-muted-foreground">
          Key
          <Input v-model="variable.key" autocomplete="off" required />
        </label>
        <label class="grid gap-2 text-xs text-muted-foreground">
          Value
          <Input
            v-model="variable.value"
            :type="variable.is_secret ? 'password' : 'text'"
            autocomplete="off"
            required
          />
        </label>
        <label
          class="grid gap-2 text-xs text-muted-foreground max-[480px]:grid-cols-[1fr_auto] max-[480px]:items-center"
        >
          Secret
          <Switch
            v-model="variable.is_secret"
            :aria-label="`Mark ${variable.key || 'variable'} secret`"
          />
        </label>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              size="icon"
              type="button"
              variant="outline"
              :aria-label="`Remove ${variable.key || 'variable'}`"
              @click="removeVariable(index)"
            >
              <Trash2 :stroke-width="1.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Remove variable</TooltipContent>
        </Tooltip>
      </div>
      <Button class="w-fit" size="sm" type="button" variant="outline" @click="addVariable">
        <Plus data-icon="inline-start" :stroke-width="1.5" />
        Add variable
      </Button>
    </fieldset>

    <Alert v-if="validationMessage" variant="destructive">
      <CircleAlert :stroke-width="1.5" />
      <AlertTitle>Configuration could not be saved</AlertTitle>
      <AlertDescription>{{ validationMessage }}</AlertDescription>
    </Alert>
    <div class="flex items-center justify-between gap-3 border-t border-border pt-4">
      <p class="text-xs text-muted-foreground">Generation {{ service.desired_generation }}</p>
      <Button type="submit" :disabled="saving">
        <Spinner v-if="saving" data-icon="inline-start" />
        {{ saving ? "Saving..." : "Save configuration" }}
      </Button>
    </div>
  </form>
</template>
