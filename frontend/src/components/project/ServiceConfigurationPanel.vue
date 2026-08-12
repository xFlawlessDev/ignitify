<script setup lang="ts">
import { Boxes, CircleAlert, Copy, Eye, EyeOff, Info, LockKeyhole } from "@lucide/vue";
import { useI18n } from "vue-i18n";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Spinner } from "@/components/ui/spinner";
import { Textarea } from "@/components/ui/textarea";
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
import ServiceEnvironmentEditor from "@/components/project/ServiceEnvironmentEditor.vue";
import { useServiceConfiguration } from "@/composables/useServiceConfiguration";
import { cn } from "@/lib/utils";
import type {
  ProjectEnvironmentVariable,
  ProviderSummary,
  ServiceInput,
  ServiceSource,
  ServiceSummary,
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

const {
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
  isGitComposeSource,
  isRepositorySource,
  kind,
  name,
  outputDirectory,
  providerId,
  removeVariable,
  repository,
  repositoryOptions,
  selectBuilder,
  selectComposeMode,
  selectedProvider,
  selectProviderEvent,
  selectRepositoryEvent,
  selectSource,
  serviceSecretCount,
  serviceVariableCount,
  showAutoDeploySecret,
  showSecretValues,
  source,
  sourceOptionClass,
  sourceOptions,
  sourceRepositories,
  sourceSummary,
  submit,
  supportsBuildCommand,
  updateSecret,
  usesStaticOutput,
  validationMessage,
} = useServiceConfiguration(props, emit);
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
            <span class="flex items-center gap-2 text-xs font-medium">
              <component
                :is="option.icon"
                :data-builder-logo="option.value"
                class="size-4 shrink-0 text-muted-foreground"
                :stroke-width="1.5"
                aria-hidden="true"
              />
              <span>{{ option.label }}</span>
            </span>
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
              <Accordion type="single" collapsible class="border-y border-border">
                <AccordionItem value="webhook-guide">
                  <AccordionTrigger class="py-3 text-xs">
                    {{ t("autoDeploy.webhookGuide.title") }}
                  </AccordionTrigger>
                  <AccordionContent class="text-xs text-muted-foreground">
                    <p class="leading-4">{{ t("autoDeploy.webhookGuide.publicEndpoint") }}</p>
                    <ol class="mt-3 grid list-decimal gap-2 pl-5 leading-4">
                      <template v-if="selectedProvider?.kind === 'github'">
                        <li>{{ t("autoDeploy.webhookGuide.github.open") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.github.credentials") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.github.events") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.github.create") }}</li>
                      </template>
                      <template v-else-if="selectedProvider?.kind === 'gitlab'">
                        <li>{{ t("autoDeploy.webhookGuide.gitlab.open") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitlab.credentials") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitlab.events") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitlab.create") }}</li>
                      </template>
                      <template v-else-if="selectedProvider?.kind === 'gitea'">
                        <li>{{ t("autoDeploy.webhookGuide.gitea.open") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitea.credentials") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitea.events") }}</li>
                        <li>{{ t("autoDeploy.webhookGuide.gitea.create") }}</li>
                      </template>
                    </ol>
                  </AccordionContent>
                </AccordionItem>
              </Accordion>
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
    <ServiceEnvironmentEditor
      :active-environment-kind="activeEnvironmentKind"
      :active-service-variables="activeServiceVariables"
      :service-secret-count="serviceSecretCount"
      :service-variable-count="serviceVariableCount"
      :show-secret-values="showSecretValues"
      @add-variable="addVariable"
      @remove-variable="removeVariable"
      @update-active-environment-kind="(value) => (activeEnvironmentKind = value)"
      @update-secret="updateSecret"
      @update-show-secret-values="(value) => (showSecretValues = value)"
    />
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
