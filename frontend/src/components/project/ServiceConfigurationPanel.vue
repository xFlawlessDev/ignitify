<script setup lang="ts">
import {
  Box,
  Boxes,
  CircleAlert,
  Copy,
  Eye,
  EyeOff,
  FileCode2,
  GitBranch,
  Info,
  LockKeyhole,
  Plus,
  Trash2,
} from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import TemplateCatalogPicker from "@/components/templates/TemplateCatalogPicker.vue";
import ComposePolicyGuide from "@/components/project/ComposePolicyGuide.vue";
import YamlCodeEditor from "@/components/project/YamlCodeEditor.vue";
import { useProviderRepositories } from "@/composables/useProviderRepositories";
import { templateRuntimeDefaults, type TemplateApplication } from "@/lib/template-catalog";
import { cn } from "@/lib/utils";
import { apiListRemoteServers, type RemoteServerSummary } from "@/lib/api";
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
  rotatingAutoDeploySecret?: boolean;
  saving?: boolean;
  service: ServiceSummary;
}>();

const emit = defineEmits<{
  save: [input: ServiceInput];
  rotateAutoDeploySecret: [];
}>();

const { t } = useI18n();

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
const autoDeploy = shallowRef(false);
const imageReference = shallowRef("");
const composeYaml = shallowRef("");
const exposedService = shallowRef("");
const internalPort = shallowRef("");
const healthcheck = shallowRef("");
const appliedTemplateName = shallowRef("");
const validationError = shallowRef<string | null>(null);
const activeEnvironmentKind = shallowRef<"variables" | "secrets">("variables");
const showSecretValues = shallowRef(false);
const showAutoDeploySecret = shallowRef(false);
const deploymentDestinationId = shallowRef("local");
const destinations = shallowRef<RemoteServerSummary[]>([]);

interface ServiceVariableDraft extends ServiceVariable {
  is_set?: boolean;
}

const variables = reactive<ServiceVariableDraft[]>([]);
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
const selectedProvider = computed(
  () => availableProviders.value.find((provider) => provider.id === providerId.value) ?? null,
);
const autoDeploySupported = computed(() =>
  ["github", "gitlab", "gitea"].includes(selectedProvider.value?.kind ?? ""),
);
const autoDeployWebhookUrl = computed(
  () => `${window.location.origin}/api/v1/webhooks/services/${props.service.id}`,
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
const isGitComposeSource = computed(
  () => source.value === "compose" && composeMode.value === "repository",
);
const serviceVariableCount = computed(
  () => variables.filter((variable) => !variable.is_secret).length,
);
const serviceSecretCount = computed(
  () => variables.filter((variable) => variable.is_secret).length,
);
const activeServiceVariables = computed(() =>
  variables
    .map((variable, index) => ({ variable, index }))
    .filter(({ variable }) =>
      activeEnvironmentKind.value === "secrets" ? variable.is_secret : !variable.is_secret,
    ),
);
const inheritedVariableCount = computed(
  () => (props.inheritedVariables ?? []).filter((variable) => !variable.is_secret).length,
);
const inheritedSecretCount = computed(
  () => (props.inheritedVariables ?? []).filter((variable) => variable.is_secret).length,
);

function isRepositorySource() {
  return (
    source.value === "application" ||
    (source.value === "compose" && composeMode.value === "repository")
  );
}

function sourceOptionClass(selected: boolean) {
  return cn(
    "grid h-auto min-h-[88px] w-full content-start justify-items-start gap-1 rounded-[6px] border px-3 py-3 text-left transition-colors",
    selected
      ? "border-[var(--status-live)] bg-muted/70"
      : "border-border hover:bg-muted/60 focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
  );
}

function builderOptionClass(selected: boolean) {
  return cn(
    "grid h-auto min-h-[64px] w-full content-start justify-items-start gap-1 rounded-[5px] border px-3 py-2 text-left transition-colors",
    selected
      ? "border-[var(--status-live)] bg-background"
      : "border-border hover:bg-muted/40 focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
  );
}

function composeModeClass(selected: boolean) {
  return cn(
    "h-auto min-h-9 w-full justify-start rounded-[5px] border px-3 py-2 text-left text-xs transition-colors",
    selected
      ? "border-[var(--status-live)] bg-muted/70"
      : "border-border hover:bg-muted/60 focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-[2px] focus-visible:outline-none",
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

function selectProviderEvent(value: string | undefined) {
  selectProvider(value ?? "");
}

function selectRepositoryEvent(value: string | undefined) {
  selectRepository(value ?? "");
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
  autoDeploy.value = props.service.source_config?.auto_deploy ?? false;
  showAutoDeploySecret.value = false;
  imageReference.value = props.service.image_reference ?? "";
  composeYaml.value = props.service.compose_yaml ?? "";
  exposedService.value = props.service.exposed_service ?? "";
  internalPort.value =
    source.value === "application" && builder.value === "static"
      ? "80"
      : (props.service.internal_port?.toString() ?? "");
  healthcheck.value = props.service.healthcheck?.join("\n") ?? "";
  deploymentDestinationId.value = props.service.deployment_destination_id ?? "local";
  validationError.value = null;
  variables.splice(
    0,
    variables.length,
    ...props.service.variables.map((variable) => ({
      key: variable.key,
      value: variable.is_secret ? "" : (variable.value ?? ""),
      is_secret: variable.is_secret,
      is_set: variable.is_set,
    })),
  );
  if (
    !variables.some((variable) =>
      activeEnvironmentKind.value === "secrets" ? variable.is_secret : !variable.is_secret,
    )
  ) {
    activeEnvironmentKind.value = variables.some((variable) => variable.is_secret)
      ? "secrets"
      : "variables";
  }
  if (providerId.value && isRepositorySource()) {
    void sourceRepositories.loadRepositories(providerId.value).then(() => {
      if (repository.value)
        void sourceRepositories.loadBranches(providerId.value, repository.value);
    });
  }
}

async function loadDestinations() {
  try {
    const result = await apiListRemoteServers();
    if (result.success) destinations.value = result.data;
  } catch {
    destinations.value = [];
  }
}

function addVariable(isSecret: boolean) {
  activeEnvironmentKind.value = isSecret ? "secrets" : "variables";
  variables.push({ key: "", value: "", is_secret: isSecret, is_set: false });
}

function selectBuilder(value: ApplicationBuilder) {
  builder.value = value;
  if (value === "static") internalPort.value = "80";
}

function removeVariable(index: number) {
  variables.splice(index, 1);
}

function updateSecret(index: number, isSecret: boolean) {
  const variable = variables[index];
  if (!variable) return;
  variable.is_secret = isSecret;
  activeEnvironmentKind.value = isSecret ? "secrets" : "variables";
}

function copyAutoDeployValue(value: string) {
  if (typeof navigator !== "undefined" && navigator.clipboard) {
    void navigator.clipboard.writeText(value);
  }
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
  activeEnvironmentKind.value = defaults.variables.some((variable) => variable.is_secret)
    ? "secrets"
    : "variables";
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
    !isGitComposeSource.value &&
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
  if (variables.some((variable) => variable.is_secret && !variable.is_set && !variable.value)) {
    validationError.value = "Enter a value for every new service secret before saving.";
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
          ...(source.value === "application"
            ? {}
            : { image_reference: imageReference.value.trim() }),
          healthcheck: healthcheckArguments.length ? healthcheckArguments : null,
        }
      : {
          ...(isGitComposeSource.value ? {} : { compose_yaml: submittedComposeYaml }),
          ...(isGitComposeSource.value ? {} : { exposed_service: exposedService.value.trim() }),
          healthcheck: null,
        }),
    internal_port: parsedPort,
    variables: variables.map(({ key, value, is_secret, is_set }) => ({
      key: key.trim(),
      value,
      is_secret,
      ...(is_secret && is_set && !value ? { preserve: true } : {}),
    })),
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
      ...(isRepositorySource() ? { auto_deploy: autoDeploy.value } : {}),
    },
    deployment_destination_id:
      deploymentDestinationId.value === "local" ? null : deploymentDestinationId.value,
  });
}

watch(() => props.service.id, reset, { immediate: true });
watch(autoDeploySupported, (supported) => {
  if (selectedProvider.value && !supported) autoDeploy.value = false;
});
onMounted(() => void loadDestinations());
</script>

<template>
  <form class="grid gap-6 rounded-[10px] border border-border bg-card p-5" @submit.prevent="submit">
    <header
      class="flex items-start justify-between gap-4 rounded-[6px] border border-border bg-muted/20 px-4 py-4 max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Service configuration</p>
        <h2 class="mt-2 text-xl font-normal">Deployment source and runtime</h2>
      </div>
      <Badge
        variant="outline"
        class="max-w-full rounded-[4px] font-mono text-[11px] text-muted-foreground"
      >
        {{ sourceSummary }}
      </Badge>
    </header>

    <div class="grid gap-2">
      <Label for="service-config-name">Service name</Label>
      <Input id="service-config-name" v-model="name" maxlength="64" required />
    </div>

    <section class="grid gap-2 border-t border-border pt-5">
      <div>
        <p class="text-sm font-medium">Deployment destination</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Local runs on this Ignitify host. Remote destinations receive releases through SSH.
        </p>
      </div>
      <Select v-model="deploymentDestinationId">
        <SelectTrigger id="service-config-destination" class="w-full">
          <SelectValue placeholder="This Ignitify host" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="local">This Ignitify host</SelectItem>
          <SelectItem
            v-for="destination in destinations"
            :key="destination.id"
            :value="destination.id"
          >
            {{ destination.name }} · {{ destination.username }}@{{ destination.host }}
          </SelectItem>
        </SelectContent>
      </Select>
    </section>

    <section class="grid gap-3 border-t border-border pt-5">
      <div>
        <p class="text-sm font-medium">Deployment source</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Choose how this service produces its runtime.
        </p>
      </div>
      <div class="grid gap-2 sm:grid-cols-3" role="group" aria-label="Deployment source">
        <Button
          variant="ghost"
          v-for="option in sourceOptions"
          :key="option.value"
          :class="cn(sourceOptionClass(source === option.value), 'text-foreground')"
          type="button"
          :aria-pressed="source === option.value"
          @click="selectSource(option.value as ServiceSource)"
        >
          <component :is="option.icon" class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <span class="text-xs font-medium">{{ option.label }}</span>
          <span class="text-[11px] text-muted-foreground">{{ option.description }}</span>
        </Button>
      </div>
    </section>

    <section
      v-if="source === 'template'"
      class="grid gap-3 rounded-[8px] border border-border bg-muted/30 p-4"
    >
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
      class="grid gap-4 rounded-[8px] border border-border bg-muted/30 p-4"
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
        <Select :model-value="providerId" required @update:model-value="selectProviderEvent">
          <SelectTrigger id="service-config-provider" class="w-full">
            <SelectValue placeholder="Select a connected provider" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="provider in availableProviders"
              :key="provider.id"
              :value="provider.id"
            >
              {{ provider.name }} ({{ provider.kind }})
            </SelectItem>
          </SelectContent>
        </Select>
        <p v-if="!availableProviders.length" class="text-[11px] text-muted-foreground">
          Connect a provider first in Providers.
        </p>
      </div>
      <div class="grid gap-3 sm:grid-cols-2">
        <div class="grid gap-2 text-xs text-muted-foreground">
          <Label for="service-config-repository">Repository</Label>
          <Select
            :model-value="repository"
            :disabled="!providerId || sourceRepositories.repositoriesLoading.value"
            required
            @update:model-value="selectRepositoryEvent"
          >
            <SelectTrigger id="service-config-repository" class="w-full">
              <SelectValue
                :placeholder="
                  sourceRepositories.repositoriesLoading.value
                    ? 'Loading repositories...'
                    : 'Select a repository'
                "
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem
                v-for="option in repositoryOptions"
                :key="option.path"
                :value="option.path"
              >
                {{ option.path }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
        <div class="grid gap-2 text-xs text-muted-foreground">
          <Label for="service-config-branch">Branch</Label>
          <Select
            v-model="branch"
            :disabled="!repository || sourceRepositories.branchesLoading.value"
            required
          >
            <SelectTrigger id="service-config-branch" class="w-full">
              <SelectValue
                :placeholder="
                  sourceRepositories.branchesLoading.value
                    ? 'Loading branches...'
                    : 'Select a branch'
                "
              />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="option in branchOptions" :key="option.name" :value="option.name">
                {{ option.name }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
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
          <Button
            variant="ghost"
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
          </Button>
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
        <Label for="service-config-build-command" class="grid gap-2 text-xs text-muted-foreground">
          Build command
          <Input
            id="service-config-build-command"
            v-model="buildCommand"
            placeholder="pnpm build"
          />
        </Label>
        <Label
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
        </Label>
      </div>
    </section>

    <section
      v-if="isRepositorySource()"
      class="grid gap-4 rounded-[8px] border border-border bg-muted/30 p-4"
    >
      <div class="flex items-start justify-between gap-4 max-[560px]:flex-col">
        <div>
          <p class="text-sm font-medium">{{ t("autoDeploy.title") }}</p>
          <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
            {{ t("autoDeploy.description") }}
          </p>
        </div>
        <Switch
          :model-value="autoDeploy"
          :disabled="!autoDeploySupported"
          :aria-label="t('autoDeploy.title')"
          @update:model-value="(value) => (autoDeploy = value)"
        />
      </div>
      <p v-if="!autoDeploySupported" class="text-[11px] leading-4 text-muted-foreground">
        {{ t("autoDeploy.unsupportedProvider") }}
      </p>
      <div v-else-if="autoDeploy" class="grid gap-3 border-t border-border pt-4">
        <p
          v-if="!service.source_config?.auto_deploy"
          class="text-[11px] leading-4 text-muted-foreground"
        >
          {{ t("autoDeploy.saveFirst") }}
        </p>
        <template v-else>
          <Label for="service-config-webhook-url" class="grid gap-2 text-xs text-muted-foreground">
            {{ t("autoDeploy.webhookUrl") }}
            <div class="flex min-w-0 gap-2">
              <Input
                id="service-config-webhook-url"
                class="min-w-0 font-mono text-xs"
                :model-value="autoDeployWebhookUrl"
                readonly
              />
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button
                    size="icon"
                    type="button"
                    variant="outline"
                    :aria-label="t('autoDeploy.copyWebhookUrl')"
                    @click="copyAutoDeployValue(autoDeployWebhookUrl)"
                  >
                    <Copy :stroke-width="1.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{{ t("autoDeploy.copyWebhookUrl") }}</TooltipContent>
              </Tooltip>
            </div>
          </Label>
          <div class="grid gap-2">
            <div class="flex items-center justify-between gap-3">
              <Label for="service-config-webhook-secret">{{ t("autoDeploy.webhookSecret") }}</Label>
              <Button
                size="sm"
                type="button"
                variant="outline"
                :disabled="rotatingAutoDeploySecret"
                @click="emit('rotateAutoDeploySecret')"
              >
                <Spinner v-if="rotatingAutoDeploySecret" data-icon="inline-start" />
                {{ t("autoDeploy.rotateSecret") }}
              </Button>
            </div>
            <template v-if="service.auto_deploy_webhook_secret">
              <div class="flex min-w-0 gap-2">
                <Input
                  id="service-config-webhook-secret"
                  class="min-w-0 font-mono text-xs"
                  :model-value="service.auto_deploy_webhook_secret"
                  :type="showAutoDeploySecret ? 'text' : 'password'"
                  readonly
                />
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button
                      size="icon"
                      type="button"
                      variant="outline"
                      :aria-label="
                        showAutoDeploySecret
                          ? t('autoDeploy.hideSecret')
                          : t('autoDeploy.revealSecret')
                      "
                      @click="showAutoDeploySecret = !showAutoDeploySecret"
                    >
                      <EyeOff v-if="showAutoDeploySecret" :stroke-width="1.5" />
                      <Eye v-else :stroke-width="1.5" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{{
                    showAutoDeploySecret ? t("autoDeploy.hideSecret") : t("autoDeploy.revealSecret")
                  }}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger as-child>
                    <Button
                      size="icon"
                      type="button"
                      variant="outline"
                      :aria-label="t('autoDeploy.copySecret')"
                      @click="copyAutoDeployValue(service.auto_deploy_webhook_secret)"
                    >
                      <Copy :stroke-width="1.5" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{{ t("autoDeploy.copySecret") }}</TooltipContent>
                </Tooltip>
              </div>
              <p class="text-[11px] leading-4 text-muted-foreground">
                {{ t("autoDeploy.secretShownOnce") }}
              </p>
            </template>
            <p v-else class="text-[11px] leading-4 text-muted-foreground">
              {{ t("autoDeploy.secretUnavailable") }}
            </p>
          </div>
        </template>
      </div>
    </section>

    <section class="grid gap-5 border-t border-border pt-5">
      <template v-if="kind === 'image'">
        <div v-if="source !== 'application'" class="grid gap-2">
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
        <ComposePolicyGuide :git-source="isGitComposeSource" />
        <div v-if="source !== 'template'" class="grid gap-2">
          <Label>Compose source</Label>
          <div class="grid gap-2 sm:grid-cols-2">
            <Button
              variant="ghost"
              :class="composeModeClass(composeMode === 'yaml')"
              type="button"
              :aria-pressed="composeMode === 'yaml'"
              @click="selectComposeMode('yaml')"
            >
              Inline YAML
            </Button>
            <Button
              variant="ghost"
              :class="composeModeClass(composeMode === 'repository')"
              type="button"
              :aria-pressed="composeMode === 'repository'"
              @click="selectComposeMode('repository')"
            >
              Provider repository
            </Button>
          </div>
        </div>
        <div
          v-if="composeMode === 'repository'"
          class="grid gap-3 rounded-[8px] border border-border bg-muted/30 p-4 sm:grid-cols-3"
        >
          <div class="grid gap-2 text-xs text-muted-foreground">
            <Label for="service-config-compose-provider">Provider</Label>
            <Select :model-value="providerId" required @update:model-value="selectProviderEvent">
              <SelectTrigger id="service-config-compose-provider" class="w-full">
                <SelectValue placeholder="Select provider" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="provider in availableProviders"
                  :key="provider.id"
                  :value="provider.id"
                >
                  {{ provider.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="grid gap-2 text-xs text-muted-foreground">
            <Label for="service-config-compose-repository">Repository</Label>
            <Select
              :model-value="repository"
              :disabled="!providerId || sourceRepositories.repositoriesLoading.value"
              required
              @update:model-value="selectRepositoryEvent"
            >
              <SelectTrigger id="service-config-compose-repository" class="w-full">
                <SelectValue
                  :placeholder="
                    sourceRepositories.repositoriesLoading.value
                      ? 'Loading repositories...'
                      : 'Select a repository'
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="option in repositoryOptions"
                  :key="option.path"
                  :value="option.path"
                >
                  {{ option.path }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="grid gap-2 text-xs text-muted-foreground">
            <Label for="service-config-compose-branch">Branch</Label>
            <Select
              v-model="branch"
              :disabled="!repository || sourceRepositories.branchesLoading.value"
              required
            >
              <SelectTrigger id="service-config-compose-branch" class="w-full">
                <SelectValue
                  :placeholder="
                    sourceRepositories.branchesLoading.value
                      ? 'Loading branches...'
                      : 'Select a branch'
                  "
                />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="option in branchOptions" :key="option.name" :value="option.name">
                  {{ option.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <Label
            class="grid gap-2 text-xs text-muted-foreground sm:col-span-3"
            for="service-config-compose-path"
          >
            Compose file path
            <Input
              id="service-config-compose-path"
              v-model="dockerfilePath"
              placeholder="docker-compose.yml"
            />
          </Label>
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
          <div class="min-w-0 app-surface">
            <div
              class="flex items-center gap-2 border-b border-border px-4 py-3 font-mono text-[11px] uppercase text-muted-foreground"
            >
              <Label
                for="service-config-compose-yaml"
                class="flex items-center gap-2 font-mono text-[11px] uppercase text-muted-foreground"
              >
                <FileCode2 class="size-3.5 text-signal" :stroke-width="1.5" aria-hidden="true" />
                docker-compose.yml
              </Label>
            </div>
            <YamlCodeEditor
              class="m-4"
              id="service-config-compose-yaml"
              v-model="composeYaml"
              aria-label="Template Compose YAML"
              placeholder="services:\n  web:\n    image: registry.example/app:1.2.3"
              required
            />
          </div>
        </div>
        <div v-if="!isGitComposeSource" class="grid gap-2">
          <Label for="service-config-exposed">Exposed Compose service</Label>
          <Input id="service-config-exposed" v-model="exposedService" placeholder="web" required />
        </div>
      </template>
    </section>

    <section class="grid gap-5 border-t border-border pt-5 sm:grid-cols-2">
      <div class="grid gap-2">
        <Label for="service-config-port">Internal port</Label>
        <Input id="service-config-port" v-model="internalPort" type="number" min="1" max="65535" />
        <p class="text-[11px] leading-4 text-muted-foreground">
          The private port exposed to the control plane.
        </p>
      </div>
      <div class="grid gap-2">
        <Label for="service-config-healthcheck">Healthcheck argv</Label>
        <Textarea
          id="service-config-healthcheck"
          v-model="healthcheck"
          placeholder="One argument per line"
          spellcheck="false"
        />
        <p class="text-[11px] leading-4 text-muted-foreground">
          Optional command arguments used to verify readiness.
        </p>
      </div>
    </section>

    <section v-if="inheritedVariables?.length" class="grid gap-2 border-t border-border pt-4">
      <div class="flex items-center gap-2">
        <Info class="size-3.5 text-muted-foreground" :stroke-width="1.5" />
        <h3 class="text-sm font-medium">Project defaults</h3>
      </div>
      <p class="text-xs leading-5 text-muted-foreground">
        {{ inheritedVariableCount }} variables and {{ inheritedSecretCount }} secrets are inherited
        at deployment. Add a matching key below to override it for this service.
      </p>
      <div class="flex flex-wrap gap-1.5">
        <Badge
          v-for="variable in inheritedVariables"
          :key="variable.key"
          variant="outline"
          class="inline-flex items-center gap-1 font-mono text-[10px] text-muted-foreground"
        >
          <LockKeyhole
            v-if="variable.is_secret"
            class="size-3 text-muted-foreground"
            :stroke-width="1.5"
          />
          {{ variable.key }}
        </Badge>
      </div>
    </section>

    <fieldset class="grid gap-3 border-t border-border pt-4">
      <legend class="sr-only">Service environment</legend>
      <div class="flex items-start justify-between gap-4 max-[560px]:flex-col">
        <div>
          <p class="text-sm font-medium">Service environment</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            Service keys override project defaults during deployment.
          </p>
        </div>
        <div class="grid shrink-0 gap-2 max-[560px]:w-full">
          <Tabs
            :model-value="activeEnvironmentKind"
            class="max-[560px]:w-full"
            @update:model-value="
              (value) => (activeEnvironmentKind = value as 'variables' | 'secrets')
            "
          >
            <TabsList class="h-8 w-full rounded-[4px] sm:w-auto">
              <TabsTrigger value="variables" class="min-w-28 px-3 text-[11px]">
                Variables
                <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                  serviceVariableCount
                }}</span>
              </TabsTrigger>
              <TabsTrigger value="secrets" class="min-w-28 px-3 text-[11px]">
                <LockKeyhole class="size-3.5" :stroke-width="1.5" />
                Secrets
                <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                  serviceSecretCount
                }}</span>
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <Button
            variant="ghost"
            v-if="activeEnvironmentKind === 'secrets' && serviceSecretCount"
            class="inline-flex items-center justify-end gap-1.5 py-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
            type="button"
            @click="showSecretValues = !showSecretValues"
          >
            <EyeOff v-if="showSecretValues" class="size-3.5" :stroke-width="1.5" />
            <Eye v-else class="size-3.5" :stroke-width="1.5" />
            {{ showSecretValues ? "Hide values" : "Reveal values" }}
          </Button>
        </div>
      </div>

      <div v-if="activeServiceVariables.length" class="grid gap-2">
        <div
          class="hidden grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] gap-2 py-1 text-[10px] uppercase text-muted-foreground sm:grid"
        >
          <span>Key</span>
          <span>Value</span>
          <span>Type</span>
          <span class="sr-only">Actions</span>
        </div>
        <div
          v-for="{ variable, index } in activeServiceVariables"
          :key="index"
          class="grid min-h-[58px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] items-end gap-2 border-b border-border py-2.5 last:border-b-0 max-[560px]:grid-cols-[minmax(0,1fr)_auto_auto]"
        >
          <Label class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground">
            Key
            <Input
              v-model="variable.key"
              class="h-8 font-mono text-xs uppercase"
              autocomplete="off"
              required
            />
          </Label>
          <Label
            class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground max-[560px]:col-span-3"
          >
            Value
            <Input
              v-model="variable.value"
              class="h-8 font-mono text-xs"
              :type="variable.is_secret && !showSecretValues ? 'password' : 'text'"
              :placeholder="
                variable.is_secret && variable.is_set
                  ? 'Stored securely; leave blank to keep'
                  : 'Enter value'
              "
              autocomplete="off"
              :required="!variable.is_secret || !variable.is_set"
            />
          </Label>
          <div class="grid gap-1.5 text-[11px] text-muted-foreground">
            Secret
            <Switch
              :model-value="variable.is_secret"
              :aria-label="'Mark ' + (variable.key || 'variable') + ' secret'"
              @update:model-value="updateSecret(index, $event)"
            />
          </div>
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                size="icon"
                type="button"
                variant="outline"
                :aria-label="'Remove ' + (variable.key || 'variable')"
                @click="removeVariable(index)"
              >
                <Trash2 :stroke-width="1.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent>Remove variable</TooltipContent>
          </Tooltip>
        </div>
      </div>
      <div v-else class="grid gap-1.5 py-4">
        <div class="flex items-center gap-2">
          <LockKeyhole
            v-if="activeEnvironmentKind === 'secrets'"
            class="size-4 text-muted-foreground"
            :stroke-width="1.5"
          />
          <p class="text-sm font-medium">
            {{
              activeEnvironmentKind === "secrets" ? "No service secrets" : "No service variables"
            }}
          </p>
        </div>
        <p class="max-w-[56ch] text-xs leading-5 text-muted-foreground">
          {{
            activeEnvironmentKind === "secrets"
              ? "Service secrets override project secrets. Stored values stay masked; leave one blank to keep its current value."
              : "Add a service-specific value when it needs to override a project variable."
          }}
        </p>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button size="sm" type="button" variant="outline" @click="addVariable(false)">
          <Plus data-icon="inline-start" :stroke-width="1.5" />
          Add variable
        </Button>
        <Button size="sm" type="button" variant="outline" @click="addVariable(true)">
          <LockKeyhole data-icon="inline-start" :stroke-width="1.5" />
          Add secret
        </Button>
      </div>
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
