<script setup lang="ts">
import { FileKey2, Plus, Trash2, Upload } from "@lucide/vue";
import { computed, reactive, shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { CustomCertificateSummary, CustomCertificateUpload } from "./types";

interface Props {
  certificates: CustomCertificateSummary[];
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (event: "add", certificate: CustomCertificateUpload): void;
  (event: "remove", certificateId: string): void;
}>();

const dialogOpen = shallowRef(false);
const certificateFile = shallowRef<File | null>(null);
const privateKeyFile = shallowRef<File | null>(null);
const showValidation = shallowRef(false);
const form = reactive({ name: "" });

const formError = computed(() => {
  if (!form.name.trim()) return "Certificate name is required.";
  if (!certificateFile.value) return "Choose a certificate file.";
  if (!privateKeyFile.value) return "Choose a private key file.";
  return "";
});

function resetForm() {
  form.name = "";
  certificateFile.value = null;
  privateKeyFile.value = null;
  showValidation.value = false;
}

function updateDialogOpen(nextOpen: boolean) {
  dialogOpen.value = nextOpen;
  if (!nextOpen) resetForm();
}

function updateFile(kind: "certificate" | "privateKey", event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0] ?? null;
  if (kind === "certificate") certificateFile.value = file;
  else privateKeyFile.value = file;
}

function addCertificate() {
  showValidation.value = true;
  if (formError.value || !certificateFile.value || !privateKeyFile.value) return;

  emit("add", {
    name: form.name.trim(),
    certificateFile: certificateFile.value,
    privateKeyFile: privateKeyFile.value,
  });
  updateDialogOpen(false);
}
</script>

<template>
  <section class="app-surface" aria-labelledby="certificates-heading">
    <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
      <div class="flex min-w-0 items-start gap-3">
        <span
          class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
        >
          <FileKey2 class="size-4" :stroke-width="1.5" />
        </span>
        <div>
          <p class="ui-label">Certificates</p>
          <h2 id="certificates-heading" class="mt-1.5 text-base font-medium">
            Custom certificates
          </h2>
          <p class="mt-1.5 max-w-[58ch] text-xs leading-5 text-muted-foreground">
            Add a certificate and private key pair for domains that use a managed custom
            certificate.
          </p>
        </div>
      </div>
      <Button class="shrink-0" size="sm" type="button" @click="updateDialogOpen(true)">
        <Plus class="size-4" :stroke-width="1.5" />
        Add certificate
      </Button>
    </header>

    <div v-if="props.certificates.length" class="divide-y divide-border">
      <article
        v-for="certificate in props.certificates"
        :key="certificate.id"
        class="flex items-center gap-3 px-5 py-3.5"
      >
        <FileKey2 class="size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div class="min-w-0 flex-1">
          <p class="truncate text-xs font-medium">{{ certificate.name }}</p>
          <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
            {{ certificate.certificateFileName }} · {{ certificate.privateKeyFileName }}
          </p>
        </div>
        <button
          class="grid size-8 shrink-0 place-items-center rounded-[3px] text-muted-foreground transition-colors hover:bg-destructive/10 hover:text-destructive"
          type="button"
          :aria-label="`Remove ${certificate.name}`"
          :title="`Remove ${certificate.name}`"
          @click="emit('remove', certificate.id)"
        >
          <Trash2 class="size-4" :stroke-width="1.5" />
        </button>
      </article>
    </div>
    <div v-else class="flex items-center gap-3 px-5 py-6 text-muted-foreground">
      <FileKey2 class="size-4 shrink-0" :stroke-width="1.5" />
      <p class="text-xs">No custom certificates added.</p>
    </div>

    <Dialog :open="dialogOpen" @update:open="updateDialogOpen">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-lg">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">Add certificate</DialogTitle>
          <DialogDescription class="text-xs leading-5">
            Select the certificate and its matching private key for a custom HTTPS domain.
          </DialogDescription>
        </DialogHeader>

        <form class="grid gap-4" @submit.prevent="addCertificate">
          <div class="grid gap-2">
            <label for="certificate-name" class="text-xs font-medium">Certificate name</label>
            <Input
              id="certificate-name"
              v-model="form.name"
              class="rounded-[3px]"
              placeholder="Production wildcard"
              autocomplete="off"
              :aria-invalid="Boolean(formError && !form.name.trim())"
            />
          </div>

          <div class="grid gap-2">
            <label for="certificate-file" class="text-xs font-medium">Certificate file</label>
            <label
              class="flex min-h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground transition-colors hover:bg-muted"
              for="certificate-file"
            >
              <Upload class="size-4 shrink-0" :stroke-width="1.5" />
              <span class="truncate">{{
                certificateFile?.name ?? "Choose .crt or .pem file"
              }}</span>
            </label>
            <input
              id="certificate-file"
              class="sr-only"
              type="file"
              accept=".crt,.pem,application/x-pem-file"
              @change="updateFile('certificate', $event)"
            />
          </div>

          <div class="grid gap-2">
            <label for="private-key-file" class="text-xs font-medium">Private key file</label>
            <label
              class="flex min-h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground transition-colors hover:bg-muted"
              for="private-key-file"
            >
              <Upload class="size-4 shrink-0" :stroke-width="1.5" />
              <span class="truncate">{{ privateKeyFile?.name ?? "Choose .key or .pem file" }}</span>
            </label>
            <input
              id="private-key-file"
              class="sr-only"
              type="file"
              accept=".key,.pem,application/x-pem-file"
              @change="updateFile('privateKey', $event)"
            />
          </div>

          <p v-if="showValidation && formError" class="text-[11px] text-destructive" role="alert">
            {{ formError }}
          </p>

          <DialogFooter class="mt-1">
            <DialogClose as-child>
              <Button variant="outline" type="button">Cancel</Button>
            </DialogClose>
            <Button type="submit">
              <Plus class="size-4" :stroke-width="1.5" />
              Add certificate
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  </section>
</template>
