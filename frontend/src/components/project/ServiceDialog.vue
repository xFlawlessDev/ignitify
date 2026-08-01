<script setup lang="ts">
import { Plus, Trash2 } from "@lucide/vue";
import { reactive, shallowRef, watch } from "vue";
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
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import type { ServiceInput, ServiceSummary, ServiceVariable } from "@/lib/types";

const props = defineProps<{
  error?: string | null;
  saving?: boolean;
  service?: ServiceSummary | null;
}>();

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ save: [input: ServiceInput] }>();

const name = shallowRef("");
const kind = shallowRef<"image" | "compose">("image");
const imageReference = shallowRef("");
const composeYaml = shallowRef("");
const exposedService = shallowRef("");
const internalPort = shallowRef("");
const healthcheck = shallowRef("");
const validationError = shallowRef<string | null>(null);
const variables = reactive<ServiceVariable[]>([]);

function reset() {
  name.value = props.service?.name ?? "";
  kind.value = props.service?.kind ?? "image";
  imageReference.value = props.service?.image_reference ?? "";
  composeYaml.value = props.service?.compose_yaml ?? "";
  exposedService.value = props.service?.exposed_service ?? "";
  internalPort.value = props.service?.internal_port?.toString() ?? "";
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

function removeVariable(index: number) {
  variables.splice(index, 1);
}

function submit() {
  if (kind.value === "image" && !imageReference.value.includes("@sha256:")) {
    validationError.value = "Image reference must include a sha256 digest.";
    return;
  }
  if (kind.value === "compose" && (!composeYaml.value.trim() || !exposedService.value.trim())) {
    validationError.value = "Compose YAML and exposed service are required.";
    return;
  }
  if (props.service && variables.some((variable) => variable.is_secret && !variable.value)) {
    validationError.value = "Re-enter each secret value before saving changes.";
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
  });
}

watch(open, (isOpen) => {
  if (isOpen) reset();
});
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent
      class="max-h-[calc(100vh-2rem)] overflow-y-auto rounded-md shadow-none sm:max-w-xl"
    >
      <DialogHeader>
        <DialogTitle>{{ service ? "Edit image service" : "New image service" }}</DialogTitle>
        <DialogDescription
          >Changes desired configuration only. Nothing deploys from this form.</DialogDescription
        >
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
        <div class="grid gap-2">
          <Label for="service-kind">Service kind</Label>
          <select
            id="service-kind"
            v-model="kind"
            class="h-9 rounded-md border border-input bg-background px-3 text-sm"
          >
            <option value="image">Digest-pinned image</option>
            <option value="compose">Hardened Compose subset</option>
          </select>
        </div>
        <div v-if="kind === 'image'" class="grid gap-2">
          <Label for="service-image">Digest-pinned image</Label>
          <Input
            id="service-image"
            v-model="imageReference"
            autocomplete="off"
            placeholder="registry.example/app@sha256:..."
            required
          />
        </div>
        <template v-else>
          <div class="grid gap-2">
            <Label for="service-compose-yaml">Compose YAML</Label>
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
        <fieldset class="grid gap-3 border-t border-border pt-4">
          <legend class="text-sm font-medium">Variables</legend>
          <div
            v-for="(variable, index) in variables"
            :key="index"
            class="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto_auto] items-end gap-2"
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
            <label class="grid gap-2 text-xs text-muted-foreground">
              Secret
              <Switch
                v-model="variable.is_secret"
                :aria-label="`Mark ${variable.key || 'variable'} secret`"
              />
            </label>
            <button
              class="grid size-9 place-items-center rounded-md border border-border text-muted-foreground hover:bg-muted hover:text-foreground"
              type="button"
              :aria-label="`Remove ${variable.key || 'variable'}`"
              title="Remove variable"
              @click="removeVariable(index)"
            >
              <Trash2 class="size-4" :stroke-width="1.5" />
            </button>
          </div>
          <Button class="w-fit" size="sm" type="button" variant="outline" @click="addVariable">
            <Plus class="size-4" :stroke-width="1.5" />
            Add variable
          </Button>
        </fieldset>
        <p v-if="validationError || error" class="text-xs text-destructive" role="alert">
          {{ validationError ?? error }}
        </p>
        <DialogFooter>
          <Button type="submit" :disabled="saving">
            {{ saving ? "Saving..." : service ? "Save configuration" : "Create service" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
