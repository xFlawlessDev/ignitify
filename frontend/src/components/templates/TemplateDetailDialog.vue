<script setup lang="ts">
import {
  BookOpen,
  ArrowLeft,
  Check,
  ClipboardCopy,
  ExternalLink,
  FileCode2,
  FileText,
  GitFork,
  Globe2,
  PackageOpen,
} from "@lucide/vue";
import { computed, onUnmounted, shallowRef, watch } from "vue";

import {
  Dialog,
  DialogClose,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogScrollContent,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import TemplateCodePreview from "@/components/templates/TemplateCodePreview.vue";
import type { TemplateApplication, TemplateMetadata } from "@/lib/template-catalog";
import { templateFileUrl } from "@/lib/template-catalog";

const props = defineProps<{
  template: TemplateMetadata | null;
}>();

const open = defineModel<boolean>("open", { default: false });
const emit = defineEmits<{
  apply: [application: TemplateApplication];
  back: [];
}>();
const compose = shallowRef<string | null>(null);
const config = shallowRef<string | null>(null);
const instructions = shallowRef<string | null>(null);
const isLoading = shallowRef(false);
const error = shallowRef<string | null>(null);
const copiedFile = shallowRef<string | null>(null);
const copyError = shallowRef<string | null>(null);
const logoFailed = shallowRef(false);

let activeController: AbortController | null = null;
let copyTimer: ReturnType<typeof setTimeout> | undefined;

const logoUrl = computed(() =>
  props.template?.logo ? templateFileUrl(props.template.id, props.template.logo) : "",
);

async function fetchTemplateFile(
  template: TemplateMetadata,
  filename: string,
  signal: AbortSignal,
): Promise<string | null> {
  const response = await fetch(templateFileUrl(template.id, filename), {
    headers: { Accept: "text/plain" },
    signal,
  });

  if (response.status === 404) return null;
  if (!response.ok) {
    throw new Error(`Unable to load ${filename} (${response.status})`);
  }

  return response.text();
}

async function loadFiles() {
  activeController?.abort();
  compose.value = null;
  config.value = null;
  instructions.value = null;
  error.value = null;
  copiedFile.value = null;
  copyError.value = null;

  if (!open.value || !props.template) {
    isLoading.value = false;
    return;
  }

  const template = props.template;
  const controller = new AbortController();
  activeController = controller;
  isLoading.value = true;

  try {
    const [composeText, configText, instructionsText] = await Promise.all([
      fetchTemplateFile(template, "docker-compose.yml", controller.signal),
      fetchTemplateFile(template, "template.toml", controller.signal),
      fetchTemplateFile(template, "instructions.md", controller.signal),
    ]);

    if (controller.signal.aborted) return;
    compose.value = composeText;
    config.value = configText;
    instructions.value = instructionsText;
  } catch (cause) {
    if (controller.signal.aborted) return;
    error.value = cause instanceof Error ? cause.message : "Unable to load template files";
  } finally {
    if (activeController === controller) {
      activeController = null;
      isLoading.value = false;
    }
  }
}

async function copyFile(filename: string, content: string | null) {
  if (!content || !navigator.clipboard) return;

  try {
    await navigator.clipboard.writeText(content);
    copiedFile.value = filename;
    if (copyTimer) clearTimeout(copyTimer);
    copyTimer = setTimeout(() => {
      if (copiedFile.value === filename) copiedFile.value = null;
    }, 1800);
  } catch {
    copyError.value = "Unable to copy this file to the clipboard";
  }
}

function applyTemplate() {
  if (!props.template || !compose.value) return;
  emit("apply", {
    template: props.template,
    composeYaml: compose.value,
    templateToml: config.value,
  });
  open.value = false;
}

function backToCatalog() {
  emit("back");
  open.value = false;
}

watch(
  [() => props.template?.id, open],
  () => {
    void loadFiles();
  },
  { immediate: true },
);

watch(
  () => props.template?.id,
  () => {
    logoFailed.value = false;
  },
);

onUnmounted(() => {
  activeController?.abort();
  if (copyTimer) clearTimeout(copyTimer);
});
</script>

<template>
  <Dialog v-model:open="open">
    <DialogScrollContent class="max-w-4xl">
      <DialogHeader v-if="template" class="pr-8">
        <div class="flex items-start gap-4">
          <div
            class="flex size-14 shrink-0 items-center justify-center overflow-hidden rounded-[6px] border border-border bg-muted p-2"
          >
            <img
              v-if="logoUrl && !logoFailed"
              :src="logoUrl"
              :alt="`${template.name} logo`"
              class="size-full object-contain"
              @error="logoFailed = true"
            />
            <PackageOpen v-else class="size-7 text-signal" aria-hidden="true" />
          </div>
          <div class="min-w-0">
            <DialogTitle class="truncate pr-2">{{ template.name }}</DialogTitle>
            <DialogDescription class="mt-1 max-w-2xl leading-6">
              {{ template.description }}
            </DialogDescription>
            <div class="mt-3 flex flex-wrap items-center gap-2">
              <span
                class="border border-border px-2 py-1 font-mono text-[10px] uppercase text-muted-foreground"
              >
                {{ template.version }}
              </span>
              <span
                v-for="tag in template.tags"
                :key="tag"
                class="border border-border px-2 py-1 font-mono text-[10px] uppercase text-muted-foreground"
              >
                {{ tag }}
              </span>
            </div>
          </div>
        </div>

        <div class="mt-5 flex flex-wrap gap-x-5 gap-y-2 border-t border-border pt-4">
          <a
            v-if="template.links.github"
            :href="template.links.github"
            target="_blank"
            rel="noreferrer"
            class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
          >
            <GitFork class="size-3.5" aria-hidden="true" />
            GitHub
            <ExternalLink class="size-3" aria-hidden="true" />
          </a>
          <a
            v-if="template.links.website"
            :href="template.links.website"
            target="_blank"
            rel="noreferrer"
            class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
          >
            <Globe2 class="size-3.5" aria-hidden="true" />
            Website
            <ExternalLink class="size-3" aria-hidden="true" />
          </a>
          <a
            v-if="template.links.docs"
            :href="template.links.docs"
            target="_blank"
            rel="noreferrer"
            class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
          >
            <BookOpen class="size-3.5" aria-hidden="true" />
            Documentation
            <ExternalLink class="size-3" aria-hidden="true" />
          </a>
        </div>
      </DialogHeader>

      <div v-if="isLoading" class="grid gap-4 py-2 lg:grid-cols-2">
        <div v-for="file in 2" :key="file" class="h-72 animate-pulse bg-muted" />
      </div>

      <div v-else-if="error" class="border border-destructive/40 bg-destructive/5 px-5 py-6">
        <p class="text-sm text-destructive">{{ error }}</p>
      </div>

      <div v-else class="space-y-4">
        <div class="grid gap-4 lg:grid-cols-2">
          <section class="min-w-0 app-surface">
            <div class="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
              <h3
                class="flex items-center gap-2 font-mono text-[11px] uppercase text-muted-foreground"
              >
                <FileCode2 class="size-3.5 text-signal" aria-hidden="true" />
                docker-compose.yml
              </h3>
              <Button
                v-if="compose"
                variant="ghost"
                size="icon-sm"
                type="button"
                :aria-label="
                  copiedFile === 'docker-compose.yml' ? 'Copied compose file' : 'Copy compose file'
                "
                :title="copiedFile === 'docker-compose.yml' ? 'Copied' : 'Copy docker-compose.yml'"
                @click="copyFile('docker-compose.yml', compose)"
              >
                <Check v-if="copiedFile === 'docker-compose.yml'" />
                <ClipboardCopy v-else />
              </Button>
            </div>
            <TemplateCodePreview
              v-if="compose"
              class="m-4"
              :content="compose"
              language="yaml"
              label="docker-compose.yml preview"
            />
            <p v-else class="px-4 py-6 text-sm text-muted-foreground">
              No compose file in this template.
            </p>
          </section>

          <section class="min-w-0 app-surface">
            <div class="flex items-center justify-between gap-3 border-b border-border px-4 py-3">
              <h3
                class="flex items-center gap-2 font-mono text-[11px] uppercase text-muted-foreground"
              >
                <FileText class="size-3.5 text-signal" aria-hidden="true" />
                template.toml
              </h3>
              <Button
                v-if="config"
                variant="ghost"
                size="icon-sm"
                type="button"
                :aria-label="
                  copiedFile === 'template.toml' ? 'Copied template config' : 'Copy template config'
                "
                :title="copiedFile === 'template.toml' ? 'Copied' : 'Copy template.toml'"
                @click="copyFile('template.toml', config)"
              >
                <Check v-if="copiedFile === 'template.toml'" />
                <ClipboardCopy v-else />
              </Button>
            </div>
            <TemplateCodePreview
              v-if="config"
              class="m-4"
              :content="config"
              language="toml"
              label="template.toml preview"
            />
            <p v-else class="px-4 py-6 text-sm text-muted-foreground">
              No template config in this template.
            </p>
          </section>
        </div>

        <section v-if="instructions" class="app-surface">
          <div class="flex items-center gap-2 border-b border-border px-4 py-3">
            <FileText class="size-3.5 text-signal" aria-hidden="true" />
            <h3 class="font-mono text-[11px] uppercase text-muted-foreground">instructions.md</h3>
          </div>
          <pre
            class="max-h-72 overflow-auto whitespace-pre-wrap p-4 text-xs leading-6 text-muted-foreground"
          ><code>{{ instructions }}</code></pre>
        </section>
      </div>

      <DialogFooter>
        <p v-if="copyError" class="mr-auto text-xs text-destructive">{{ copyError }}</p>
        <Button class="mr-auto" variant="ghost" type="button" @click="backToCatalog">
          <ArrowLeft data-icon="inline-start" />
          Back to templates
        </Button>
        <Button
          v-if="template"
          type="button"
          :disabled="isLoading || !compose"
          @click="applyTemplate"
        >
          Apply template
        </Button>
        <DialogClose as-child>
          <Button variant="outline" type="button">Close</Button>
        </DialogClose>
      </DialogFooter>
    </DialogScrollContent>
  </Dialog>
</template>
