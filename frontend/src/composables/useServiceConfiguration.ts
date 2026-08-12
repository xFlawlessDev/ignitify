import { computed, onMounted, reactive, shallowRef, watch, type Component } from "vue";
import { Box, Container, FileCode2, GitBranch, Globe2, Rocket } from "@lucide/vue";
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

export interface ServiceConfigurationProps {
  error?: string | null;
  inheritedVariables?: ProjectEnvironmentVariable[];
  providers?: ProviderSummary[];
  rotatingAutoDeploySecret?: boolean;
  saving?: boolean;
  service: ServiceSummary;
}

export interface ServiceConfigurationEmit {
  (event: "save", input: ServiceInput): void;
  (event: "rotateAutoDeploySecret"): void;
}

interface ServiceVariableDraft extends ServiceVariable {
  is_set?: boolean;
}

export function useServiceConfiguration(
  props: ServiceConfigurationProps,
  emit: ServiceConfigurationEmit,
) {
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
  const variables = reactive<ServiceVariableDraft[]>([]);
  const sourceRepositories = useProviderRepositories();
  const builderOptions: Array<{
    value: ApplicationBuilder;
    label: string;
    description: string;
    icon: Component;
  }> = [
    {
      value: "static",
      label: "Static",
      description: "Build and serve static assets",
      icon: Globe2,
    },
    {
      value: "dockerfile",
      label: "Dockerfile",
      description: "Use the repository Dockerfile",
      icon: Container,
    },
    { value: "railpack", label: "Railpack", description: "Detect and build the app", icon: Rocket },
  ];
  const sourceOptions = [
    { value: "template" as const, label: "Template", icon: Box, description: "Catalog runtime" },
    { value: "compose" as const, label: "Compose", icon: FileCode2, description: "Managed YAML" },
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
    if (source.value === "application") return repository.value.trim() || `${builder.value} build`;
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
      source.value === "application" &&
      (builder.value === "static" || builder.value === "railpack"),
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
    return source.value === "application" || isGitComposeSource.value;
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
    if (value !== "template") appliedTemplateName.value = "";
    if (value === "compose" && dockerfilePath.value === "Dockerfile")
      dockerfilePath.value = "docker-compose.yml";
    if (value === "application" && builder.value === "static") internalPort.value = "80";
    if (!isRepositorySource()) sourceRepositories.reset();
  }
  function selectComposeMode(value: "yaml" | "repository") {
    composeMode.value = value;
    validationError.value = null;
    if (value === "repository" && dockerfilePath.value === "Dockerfile")
      dockerfilePath.value = "docker-compose.yml";
    if (value === "repository" && providerId.value) {
      void sourceRepositories.loadRepositories(providerId.value);
      return;
    }
    if (value === "yaml") sourceRepositories.reset();
  }
  const selectProviderEvent = (value: string | undefined) => selectProvider(value ?? "");
  const selectRepositoryEvent = (value: string | undefined) => selectRepository(value ?? "");

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
  const addVariable = (isSecret: boolean) => {
    activeEnvironmentKind.value = isSecret ? "secrets" : "variables";
    variables.push({ key: "", value: "", is_secret: isSecret, is_set: false });
  };
  const selectBuilder = (value: ApplicationBuilder) => {
    builder.value = value;
    if (value === "static") internalPort.value = "80";
  };
  const removeVariable = (index: number) => variables.splice(index, 1);
  function updateSecret(index: number, isSecret: boolean) {
    const variable = variables[index];
    if (!variable) return;
    variable.is_secret = isSecret;
    activeEnvironmentKind.value = isSecret ? "secrets" : "variables";
  }
  const copyAutoDeployValue = (value: string) => {
    if (typeof navigator !== "undefined" && navigator.clipboard)
      void navigator.clipboard.writeText(value);
  };
  const normalizeComposeYaml = (value: string) =>
    value.replace(/\r\n/g, "\n").split(String.fromCharCode(0)).join("");
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
  const isDigestPinnedImage = (value: string) => /^[^\s@]+@sha256:[a-fA-F0-9]{64}$/.test(value);

  function submit() {
    validationError.value = null;
    const submittedComposeYaml = normalizeComposeYaml(composeYaml.value);
    composeYaml.value = submittedComposeYaml;
    if (source.value === "template" && !submittedComposeYaml.trim())
      return void (validationError.value = "Choose a template before saving changes.");
    if (
      source.value !== "application" &&
      source.value !== "template" &&
      kind.value === "image" &&
      !isDigestPinnedImage(imageReference.value.trim())
    )
      return void (validationError.value = "Image reference must include an exact sha256 digest.");
    if (
      kind.value === "compose" &&
      !isGitComposeSource.value &&
      (!exposedService.value.trim() ||
        (composeMode.value === "yaml" && !submittedComposeYaml.trim()))
    )
      return void (validationError.value = "Compose YAML and exposed service are required.");
    if (
      source.value === "compose" &&
      composeMode.value === "repository" &&
      (!providerId.value || !repository.value.trim() || !branch.value.trim())
    )
      return void (validationError.value =
        "Choose a provider, repository, and branch for the Compose file.");
    if (variables.some((variable) => variable.is_secret && !variable.is_set && !variable.value))
      return void (validationError.value =
        "Enter a value for every new service secret before saving.");
    if (
      source.value === "application" &&
      (!providerId.value || !repository.value.trim() || !branch.value.trim())
    )
      return void (validationError.value =
        "Choose a provider, repository, and branch for the application.");
    const port = String(internalPort.value).trim();
    const parsedPort = port ? Number(port) : null;
    if (
      parsedPort !== null &&
      (!Number.isInteger(parsedPort) || parsedPort < 1 || parsedPort > 65535)
    )
      return void (validationError.value = "Internal port must be between 1 and 65535.");
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

  return {
    activeEnvironmentKind,
    activeServiceVariables,
    addVariable,
    appliedTemplateName,
    applyTemplate,
    autoDeploy,
    autoDeploySupported,
    autoDeployWebhookUrl,
    availableProviders,
    branch,
    branchOptions,
    buildCommand,
    builder,
    builderOptionClass,
    builderOptions,
    composeMode,
    composeModeClass,
    composeYaml,
    copyAutoDeployValue,
    deploymentDestinationId,
    destinations,
    dockerfilePath,
    exposedService,
    healthcheck,
    imageReference,
    inheritedSecretCount,
    inheritedVariableCount,
    internalPort,
    isDigestPinnedImage,
    isGitComposeSource,
    isRepositorySource,
    kind,
    name,
    normalizeComposeYaml,
    outputDirectory,
    providerId,
    removeVariable,
    repository,
    repositoryOptions,
    reset,
    selectBuilder,
    selectComposeMode,
    selectedProvider,
    selectProvider,
    selectProviderEvent,
    selectRepository,
    selectRepositoryEvent,
    selectSource,
    serviceSecretCount,
    serviceVariableCount,
    showAutoDeploySecret,
    showSecretValues,
    source,
    sourceOptions,
    sourceOptionClass,
    sourceRepositories,
    sourceSummary,
    submit,
    supportsBuildCommand,
    template,
    updateSecret,
    usesStaticOutput,
    validationError,
    validationMessage,
    variables,
  };
}
