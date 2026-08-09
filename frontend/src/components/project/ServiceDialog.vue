<script setup lang="ts">
import { Box, Boxes, FileCode2, GitBranch, Info, Plus, Trash2 } from "@lucide/vue";
import { computed, reactive, shallowRef, watch } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type {
  ProjectEnvironmentVariable,
  ApplicationBuilder,
  ProviderSummary,
  ServiceInput,
  ServiceSource,
  ServiceSummary,
  ServiceVariable,
} from "@/lib/types";

const STARTER_IMAGE_REFERENCE =
  "caddy:2.11.4-alpine@sha256:98eb57d882ccd5213d1688764db10c1ca2c58a1ca3a6717a3411ad798f7a423a";

const props = defineProps<{
  error?: string | null;
  saving?: boolean;
  service?: ServiceSummary | null;
  inheritedVariables?: ProjectEnvironmentVariable[];
  providers?: ProviderSummary[];
}>();

const open = defineModel<boolean>("open", { required: true });
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
const validationError = shallowRef<string | null>(null);
const variables = reactive<ServiceVariable[]>([]);

const templateOptions = [
  { value: "static", label: "Static site", description: "Nginx serves a static directory" },
  { value: "spa", label: "SPA", description: "Single-page app with history fallback" },
  { value: "node", label: "Node service", description: "A ready-to-run Node container" },
] as const;
const builderOptions: Array<{ value: ApplicationBuilder; label: string; description: string }> = [
  { value: "static", label: "Static", description: "Build and serve static assets" },
  { value: "dockerfile", label: "Dockerfile", description: "Use the repository Dockerfile" },
  { value: "railpack", label: "Railpack", description: "Detect and build the app" },
];
const availableProviders = computed(() =>
  (props.providers ?? []).filter((provider) => provider.token_configured),
);

function reset() {
  name.value = props.service?.name ?? "";
  kind.value = props.service?.kind ?? "image";
  source.value =
    props.service?.source_config?.source ?? (kind.value === "compose" ? "compose" : "template");
  composeMode.value = props.service?.source_config?.provider_id ? "repository" : "yaml";
  template.value = props.service?.source_config?.setup_required
    ? "static"
    : (props.service?.source_config?.template ?? "static");
  providerId.value = props.service?.source_config?.provider_id ?? "";
  repository.value = props.service?.source_config?.repository ?? "";
  branch.value = props.service?.source_config?.branch ?? "main";
  builder.value =
    props.service?.source_config?.builder === "spa"
      ? "static"
      : (props.service?.source_config?.builder ?? "static");
  dockerfilePath.value =
    props.service?.source_config?.dockerfile_path ??
    (props.service?.source_config?.source === "compose" ? "docker-compose.yml" : "Dockerfile");
  buildCommand.value = props.service?.source_config?.build_command ?? "";
  outputDirectory.value = props.service?.source_config?.output_directory ?? "dist";
  imageReference.value = props.service?.image_reference ?? "";
  composeYaml.value = props.service?.compose_yaml ?? "";
  exposedService.value = props.service?.exposed_service ?? "";
  internalPort.value =
    source.value === "application" && builder.value === "static"
      ? "80"
      : (props.service?.internal_port?.toString() ?? "");
  healthcheck.value = props.service?.healthcheck?.join("\n") ?? "";
  validationError.value = null;
  variables.splice(
    0,
    variables.length,
    ...(props.service?.variables.map((variable) => ({
      key: variable.key,
      value: variable.is_secret ? "" : (variable.value ?? ""),
      is_secret: variable.is_secret,
    })) ?? []),
  );
}

function addVariable() {
  variables.push({ key: "", value: "", is_secret: true });
}

function selectBuilder(value: ApplicationBuilder) {
  builder.value = value;
  if (value === "static") internalPort.value = "80";
}

function selectApplicationSource() {
  source.value = "application";
  kind.value = "image";
  if (builder.value === "static") internalPort.value = "80";
}

function removeVariable(index: number) {
  variables.splice(index, 1);
}

function isDigestPinnedImage(value: string) {
  return /^[^\s@]+@sha256:[a-fA-F0-9]{64}$/.test(value);
}

function submit() {
  if (!props.service) {
    emit("save", {
      name: name.value.trim(),
      kind: "image",
      image_reference: STARTER_IMAGE_REFERENCE,
      internal_port: 80,
      healthcheck: null,
      variables: [],
      source_config: {
        source: "template",
        template: "starter",
        setup_required: true,
      },
    });
    return;
  }
  if (
    source.value !== "application" &&
    kind.value === "image" &&
    !isDigestPinnedImage(imageReference.value.trim())
  ) {
    validationError.value = "Image reference must include an exact sha256 digest.";
    return;
  }
  if (
    kind.value === "compose" &&
    (!exposedService.value.trim() || (composeMode.value === "yaml" && !composeYaml.value.trim()))
  ) {
    validationError.value = "Compose YAML and exposed service are required.";
    return;
  }
  if (
    source.value === "compose" &&
    composeMode.value === "repository" &&
    (!providerId.value || !repository.value.trim())
  ) {
    validationError.value = "Choose a provider and repository for the Compose file.";
    return;
  }
  if (props.service && variables.some((variable) => variable.is_secret && !variable.value)) {
    validationError.value = "Re-enter each secret value before saving changes.";
    return;
  }
  if (source.value === "application" && (!providerId.value || !repository.value.trim())) {
    validationError.value = "Choose a provider and repository for the application.";
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
          compose_yaml: composeYaml.value,
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
            branch: branch.value.trim() || "main",
            builder: builder.value,
            ...(dockerfilePath.value.trim()
              ? { dockerfile_path: dockerfilePath.value.trim() }
              : {}),
            ...(buildCommand.value.trim() ? { build_command: buildCommand.value.trim() } : {}),
            ...(outputDirectory.value.trim()
              ? { output_directory: outputDirectory.value.trim() }
              : {}),
          }
        : {}),
      ...(source.value === "compose" && composeMode.value === "repository"
        ? {
            provider_id: providerId.value,
            repository: repository.value.trim(),
            branch: branch.value.trim() || "main",
            ...(dockerfilePath.value.trim()
              ? { dockerfile_path: dockerfilePath.value.trim() }
              : {}),
          }
        : {}),
    },
  });
}

watch(open, (isOpen) => {
  if (isOpen) reset();
});
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      class="max-h-[calc(100vh-2rem)] w-[calc(100%-1rem)] overflow-y-auto rounded-[10px] shadow-none sm:max-w-xl"
    >
      <DialogHeader>
        <DialogTitle>{{ service ? "Edit service" : "New deployment service" }}</DialogTitle>
        <DialogDescription>
          {{
            service
              ? "Changes desired configuration only. Nothing deploys from this form."
              : "Create the service record first. Configure its deployment from the service detail."
          }}
        </DialogDescription>
      </DialogHeader>
      <form class="grid gap-5" @submit.prevent="submit">
        <div class="grid gap-2">
          <Label for="service-name">Service name</Label>
          <Input
            id="service-name"
            v-model="name"
            autocomplete="off"
            maxlength="64"
            pattern="[a-z0-9]([a-z0-9-]*[a-z0-9])?"
            required
          />
        </div>
        <p v-if="!service" class="text-xs leading-5 text-muted-foreground">
          A stopped starter service will be created. Configure its source, runtime, and environment
          from the service detail before deploying.
        </p>
        <template v-if="service">
          <div class="grid gap-2">
            <Label for="service-kind">Deployment source</Label>
            <div class="grid gap-2 sm:grid-cols-3" role="group" aria-label="Deployment source">
              <Button
                variant="ghost"
                class="grid gap-1 rounded-[6px] border px-3 py-2 text-left transition-colors"
                :class="
                  source === 'template'
                    ? 'border-[var(--status-live)] bg-muted'
                    : 'border-border hover:bg-muted'
                "
                type="button"
                :aria-pressed="source === 'template'"
                @click="
                  source = 'template';
                  kind = 'image';
                "
              >
                <Box class="size-4 text-muted-foreground" :stroke-width="1.5" />
                <span class="text-xs font-medium">Template</span>
                <span class="text-[11px] text-muted-foreground">Start from a known shape</span>
              </Button>
              <Button
                variant="ghost"
                class="grid gap-1 rounded-[6px] border px-3 py-2 text-left transition-colors"
                :class="
                  source === 'compose'
                    ? 'border-[var(--status-live)] bg-muted'
                    : 'border-border hover:bg-muted'
                "
                type="button"
                :aria-pressed="source === 'compose'"
                @click="
                  source = 'compose';
                  kind = 'compose';
                  if (dockerfilePath === 'Dockerfile') dockerfilePath = 'docker-compose.yml';
                "
              >
                <FileCode2 class="size-4 text-muted-foreground" :stroke-width="1.5" />
                <span class="text-xs font-medium">Compose</span>
                <span class="text-[11px] text-muted-foreground">Paste a hardened YAML file</span>
              </Button>
              <Button
                variant="ghost"
                class="grid gap-1 rounded-[6px] border px-3 py-2 text-left transition-colors"
                :class="
                  source === 'application'
                    ? 'border-[var(--status-live)] bg-muted'
                    : 'border-border hover:bg-muted'
                "
                type="button"
                :aria-pressed="source === 'application'"
                @click="selectApplicationSource"
              >
                <GitBranch class="size-4 text-muted-foreground" :stroke-width="1.5" />
                <span class="text-xs font-medium">Application</span>
                <span class="text-[11px] text-muted-foreground">Build from a provider repo</span>
              </Button>
            </div>
            <Select v-model="kind">
              <SelectTrigger id="service-kind" class="sr-only" aria-hidden="true" tabindex="-1">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="image">Container image</SelectItem>
                <SelectItem value="compose">Raw Compose / Docker</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <section
            v-if="source === 'template'"
            class="grid gap-3 rounded-[8px] border border-border bg-muted/30 p-4"
          >
            <div class="flex items-start gap-2">
              <Boxes class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
              <div>
                <p class="text-xs font-medium">Choose a template</p>
                <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
                  The template only fills defaults. The runtime image remains digest-pinned.
                </p>
              </div>
            </div>
            <div class="grid gap-2 sm:grid-cols-3">
              <Button
                variant="ghost"
                v-for="option in templateOptions"
                :key="option.value"
                class="grid gap-1 rounded-[5px] border px-3 py-2 text-left"
                :class="
                  template === option.value
                    ? 'border-[var(--status-live)] bg-background'
                    : 'border-border hover:bg-background'
                "
                type="button"
                :aria-pressed="template === option.value"
                @click="template = option.value"
              >
                <span class="text-xs font-medium">{{ option.label }}</span>
                <span class="text-[11px] leading-4 text-muted-foreground">{{
                  option.description
                }}</span>
              </Button>
            </div>
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
              <Label for="service-provider">Repository provider</Label>
              <Select v-model="providerId" required>
                <SelectTrigger id="service-provider" class="w-full">
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
              <Label for="service-repository" class="grid gap-2 text-xs text-muted-foreground"
                >Repository<Input
                  id="service-repository"
                  v-model="repository"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="owner/repository"
                  required
              /></Label>
              <Label for="service-branch" class="grid gap-2 text-xs text-muted-foreground"
                >Branch<Input
                  id="service-branch"
                  v-model="branch"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="main"
              /></Label>
            </div>
            <div class="grid gap-2">
              <Label>Builder</Label>
              <div class="grid gap-2 sm:grid-cols-2">
                <Button
                  variant="ghost"
                  v-for="option in builderOptions"
                  :key="option.value"
                  class="grid gap-1 rounded-[5px] border px-3 py-2 text-left"
                  :class="
                    builder === option.value
                      ? 'border-[var(--status-live)] bg-background'
                      : 'border-border hover:bg-background'
                  "
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
              <Label for="service-dockerfile">Dockerfile path</Label>
              <Input id="service-dockerfile" v-model="dockerfilePath" placeholder="Dockerfile" />
            </div>
            <div class="grid gap-3 sm:grid-cols-2">
              <Label for="service-build-command" class="grid gap-2 text-xs text-muted-foreground"
                >Build command<Input
                  id="service-build-command"
                  v-model="buildCommand"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="pnpm build"
              /></Label>
              <Label for="service-output-directory" class="grid gap-2 text-xs text-muted-foreground"
                >Output directory<Input
                  id="service-output-directory"
                  v-model="outputDirectory"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="dist"
              /></Label>
            </div>
          </section>
          <div v-if="kind === 'image'" class="grid gap-2">
            <Label for="service-image">Runtime image digest</Label>
            <Input
              id="service-image"
              v-model="imageReference"
              autocomplete="off"
              placeholder="registry.example/app@sha256:..."
              :required="source !== 'application'"
            />
          </div>
          <template v-else>
            <div class="grid gap-2">
              <Label>Compose source</Label>
              <div class="grid gap-2 sm:grid-cols-2">
                <Button
                  variant="ghost"
                  class="rounded-[5px] border px-3 py-2 text-left text-xs"
                  :class="
                    composeMode === 'yaml'
                      ? 'border-[var(--status-live)] bg-muted'
                      : 'border-border hover:bg-muted'
                  "
                  type="button"
                  :aria-pressed="composeMode === 'yaml'"
                  @click="composeMode = 'yaml'"
                >
                  Inline YAML
                </Button>
                <Button
                  variant="ghost"
                  class="rounded-[5px] border px-3 py-2 text-left text-xs"
                  :class="
                    composeMode === 'repository'
                      ? 'border-[var(--status-live)] bg-muted'
                      : 'border-border hover:bg-muted'
                  "
                  type="button"
                  :aria-pressed="composeMode === 'repository'"
                  @click="
                    composeMode = 'repository';
                    if (dockerfilePath === 'Dockerfile') dockerfilePath = 'docker-compose.yml';
                  "
                >
                  Provider repository
                </Button>
              </div>
            </div>
            <div
              v-if="composeMode === 'repository'"
              class="grid gap-3 rounded-[8px] border border-border bg-muted/30 p-4 sm:grid-cols-3"
            >
              <div class="grid gap-2 text-xs text-muted-foreground sm:col-span-1">
                <Label for="service-provider">Provider</Label>
                <Select v-model="providerId" required>
                  <SelectTrigger id="service-provider" class="w-full text-foreground">
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
              <Label
                class="grid gap-2 text-xs text-muted-foreground sm:col-span-1"
                for="service-repository"
                >Repository<Input
                  id="service-repository"
                  v-model="repository"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="owner/repository"
                  required
              /></Label>
              <Label
                class="grid gap-2 text-xs text-muted-foreground sm:col-span-1"
                for="service-branch"
                >Branch<Input
                  id="service-branch"
                  v-model="branch"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="main"
              /></Label>
              <Label
                class="grid gap-2 text-xs text-muted-foreground sm:col-span-3"
                for="service-compose-path"
                >Compose file path<Input
                  id="service-compose-path"
                  v-model="dockerfilePath"
                  class="h-9 rounded-[3px] border border-input bg-background px-3 text-sm text-foreground"
                  placeholder="docker-compose.yml"
              /></Label>
            </div>
            <div v-if="composeMode === 'yaml'" class="grid gap-2">
              <Label for="service-compose-yaml">Compose / Docker file</Label>
              <Textarea
                id="service-compose-yaml"
                v-model="composeYaml"
                class="min-h-48 font-mono text-xs"
                spellcheck="false"
                placeholder="services:\n  web:\n    image: registry.example/app@sha256:..."
                required
              />
              <p class="text-xs text-muted-foreground">
                Prebuilt images only. No builds, host ports, binds, privileged mode, devices, or raw
                Traefik labels.
              </p>
            </div>
            <div class="grid gap-2">
              <Label for="service-exposed">Exposed Compose service</Label>
              <Input id="service-exposed" v-model="exposedService" placeholder="web" required />
            </div>
          </template>
          <div class="grid gap-2">
            <Label for="service-port">Internal port</Label>
            <Input id="service-port" v-model="internalPort" type="number" min="1" max="65535" />
          </div>
          <div class="grid gap-2">
            <Label for="service-healthcheck">Healthcheck argv</Label>
            <Textarea
              id="service-healthcheck"
              v-model="healthcheck"
              placeholder="One argument per line"
              spellcheck="false"
            />
          </div>
          <section
            v-if="inheritedVariables?.length"
            class="grid gap-2 border-t border-border pt-4"
            aria-labelledby="service-inherited-title"
          >
            <div class="flex items-center gap-2">
              <Info class="size-3.5 text-muted-foreground" :stroke-width="1.5" />
              <h3 id="service-inherited-title" class="text-sm font-medium">Project defaults</h3>
            </div>
            <p class="text-xs leading-5 text-muted-foreground">
              The control plane injects these keys at deployment. Add a matching key below to
              override it for this service.
            </p>
            <div class="flex flex-wrap gap-1.5">
              <span
                v-for="variable in inheritedVariables"
                :key="variable.key"
                class="border border-border bg-muted px-2 py-1 font-mono text-[10px] text-muted-foreground"
              >
                {{ variable.key }}
              </span>
            </div>
          </section>
          <fieldset class="grid gap-3 border-t border-border pt-4">
            <legend class="text-sm font-medium">Service overrides</legend>
            <p class="-mt-1 text-xs leading-5 text-muted-foreground">
              Keep values that only this service needs here. They override project defaults during
              deployment.
            </p>
            <div
              v-for="(variable, index) in variables"
              :key="index"
              class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-end gap-2 max-[480px]:grid-cols-1 max-[480px]:items-stretch"
            >
              <Label class="grid gap-2 text-xs text-muted-foreground">
                Key
                <Input v-model="variable.key" autocomplete="off" required />
              </Label>
              <Label class="grid gap-2 text-xs text-muted-foreground">
                Value
                <Input
                  v-model="variable.value"
                  :type="variable.is_secret ? 'password' : 'text'"
                  autocomplete="off"
                  required
                />
              </Label>
              <div
                class="grid gap-2 text-xs text-muted-foreground max-[480px]:grid-cols-[1fr_auto] max-[480px]:items-center"
              >
                Secret
                <Switch
                  v-model="variable.is_secret"
                  :aria-label="`Mark ${variable.key || 'variable'} secret`"
                />
              </div>
              <Button
                variant="ghost"
                class="grid size-9 place-items-center rounded-[3px] border border-border text-muted-foreground hover:bg-muted hover:text-foreground"
                type="button"
                :aria-label="`Remove ${variable.key || 'variable'}`"
                title="Remove variable"
                @click="removeVariable(index)"
              >
                <Trash2 class="size-4" :stroke-width="1.5" />
              </Button>
            </div>
            <Button class="w-fit" size="sm" type="button" variant="outline" @click="addVariable">
              <Plus class="size-4" :stroke-width="1.5" />
              Add variable
            </Button>
          </fieldset>
        </template>
        <p v-if="validationError || error" class="text-xs text-destructive" role="alert">
          {{ validationError ?? error }}
        </p>
        <DialogFooter class="flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <Button class="w-full sm:w-auto" type="submit" :disabled="saving">
            {{ saving ? "Saving..." : service ? "Save configuration" : "Create service" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
