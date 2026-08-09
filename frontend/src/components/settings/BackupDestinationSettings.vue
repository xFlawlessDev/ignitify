<script setup lang="ts">
import { Cloud, Pencil, Save, Trash2, X } from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  apiDeleteBackupS3Destination,
  apiGetBackupS3Destination,
  apiUpdateBackupS3Destination,
} from "@/lib/api";
import type { BackupS3Destination, S3ServerSideEncryption } from "@/lib/api/backup-destinations";

interface S3DestinationDraft {
  endpoint: string;
  region: string;
  bucket: string;
  prefix: string;
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken: string;
  serverSideEncryption: S3ServerSideEncryption;
}

const destination = shallowRef<BackupS3Destination | null>(null);
const editing = shallowRef(false);
const state = shallowRef<"loading" | "idle" | "saving" | "removing" | "error">("loading");
const requestError = shallowRef("");
const draft = reactive<S3DestinationDraft>(emptyDraft());

const validationError = computed(() => {
  const endpoint = draft.endpoint.trim();
  try {
    const url = new URL(endpoint);
    if (
      url.protocol !== "https:" ||
      !url.hostname ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      return "Use an HTTPS S3 endpoint without a path.";
    }
  } catch {
    return "Use an HTTPS S3 endpoint without a path.";
  }
  if (!/^[a-z0-9-]{1,64}$/.test(draft.region.trim())) return "Use a valid S3 region.";
  if (!validBucket(draft.bucket.trim())) return "Use a valid S3 bucket name.";
  if (!validPrefix(draft.prefix.trim())) return "Use a valid backup prefix.";
  if (!validCredential(draft.accessKeyId, 128)) return "Enter an S3 access key ID.";
  if (!validCredential(draft.secretAccessKey, 256)) return "Enter an S3 secret access key.";
  if (draft.sessionToken.trim() && !validCredential(draft.sessionToken, 4096)) {
    return "S3 session token is invalid.";
  }
  return "";
});

const canSave = computed(
  () => state.value !== "saving" && state.value !== "removing" && !validationError.value,
);

function emptyDraft(): S3DestinationDraft {
  return {
    endpoint: "",
    region: "us-east-1",
    bucket: "",
    prefix: "ignitify",
    accessKeyId: "",
    secretAccessKey: "",
    sessionToken: "",
    serverSideEncryption: "AES256",
  };
}

function applyDestination(value: BackupS3Destination | null) {
  destination.value = value;
  if (!value) {
    Object.assign(draft, emptyDraft());
    return;
  }
  Object.assign(draft, {
    endpoint: value.endpoint,
    region: value.region,
    bucket: value.bucket,
    prefix: value.prefix,
    accessKeyId: "",
    secretAccessKey: "",
    sessionToken: "",
    serverSideEncryption: value.server_side_encryption,
  });
}

function beginEditing() {
  editing.value = true;
  requestError.value = "";
  state.value = "idle";
}

function cancelEditing() {
  editing.value = false;
  requestError.value = "";
  applyDestination(destination.value);
  state.value = "idle";
}

async function load() {
  state.value = "loading";
  requestError.value = "";
  const result = await apiGetBackupS3Destination();
  if (!result.success) {
    requestError.value = result.error ?? "Unable to load backup destination.";
    state.value = "error";
    return;
  }
  applyDestination(result.data);
  editing.value = result.data === null;
  state.value = "idle";
}

async function save() {
  if (!canSave.value) return;
  state.value = "saving";
  requestError.value = "";
  const result = await apiUpdateBackupS3Destination({
    endpoint: draft.endpoint.trim().replace(/\/$/, ""),
    region: draft.region.trim().toLowerCase(),
    bucket: draft.bucket.trim().toLowerCase(),
    prefix: draft.prefix.trim().replace(/^\/+|\/+$/g, ""),
    access_key_id: draft.accessKeyId.trim(),
    secret_access_key: draft.secretAccessKey.trim(),
    ...(draft.sessionToken.trim() ? { session_token: draft.sessionToken.trim() } : {}),
    server_side_encryption: draft.serverSideEncryption,
  });
  if (!result.success) {
    requestError.value = result.error ?? "Unable to save backup destination.";
    state.value = "error";
    return;
  }
  applyDestination(result.data);
  editing.value = false;
  state.value = "idle";
}

async function remove() {
  if (
    !window.confirm("Remove the S3 backup destination? Existing backup objects are not deleted.")
  ) {
    return;
  }
  state.value = "removing";
  requestError.value = "";
  const result = await apiDeleteBackupS3Destination();
  if (!result.success) {
    requestError.value = result.error ?? "Unable to remove backup destination.";
    state.value = "error";
    return;
  }
  applyDestination(null);
  editing.value = true;
  state.value = "idle";
}

function validBucket(value: string) {
  return (
    /^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$/.test(value) &&
    !value.includes("..") &&
    !value.startsWith("-") &&
    !value.endsWith("-")
  );
}

function validPrefix(value: string) {
  if (!value) return true;
  return value.split("/").every((segment) => /^[a-z0-9][a-z0-9._-]*$/.test(segment));
}

function validCredential(value: string, maximum: number) {
  const trimmed = value.trim();
  return trimmed.length > 0 && trimmed.length <= maximum && !/\s/.test(trimmed);
}

onMounted(load);
</script>

<template>
  <section class="app-surface" aria-labelledby="backup-destination-heading">
    <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
      <div class="flex items-start gap-3">
        <span
          class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
        >
          <Cloud class="size-4" :stroke-width="1.5" />
        </span>
        <div>
          <p class="ui-label">Backup destination</p>
          <h2 id="backup-destination-heading" class="mt-1.5 text-base font-medium">
            S3-compatible storage
          </h2>
        </div>
      </div>
      <span
        class="shrink-0 font-mono text-[10px]"
        :class="destination ? 'text-metric-green' : 'text-muted-foreground'"
      >
        {{ destination ? "configured" : state === "loading" ? "checking" : "not configured" }}
      </span>
    </header>

    <div
      v-if="state === 'loading'"
      class="border-t border-border px-5 py-4 text-xs text-muted-foreground"
    >
      Loading backup destination
    </div>

    <div v-else-if="destination && !editing" class="border-t border-border">
      <dl class="grid divide-y divide-border sm:grid-cols-3 sm:divide-x sm:divide-y-0">
        <div class="min-w-0 px-5 py-4">
          <dt class="text-xs font-medium">Endpoint</dt>
          <dd class="mt-1 break-all font-mono text-[11px] text-muted-foreground">
            {{ destination.endpoint }}
          </dd>
        </div>
        <div class="min-w-0 px-5 py-4">
          <dt class="text-xs font-medium">Bucket</dt>
          <dd class="mt-1 break-all font-mono text-[11px] text-muted-foreground">
            {{ destination.bucket }}<span v-if="destination.prefix">/{{ destination.prefix }}</span>
          </dd>
        </div>
        <div class="min-w-0 px-5 py-4">
          <dt class="text-xs font-medium">Encryption</dt>
          <dd class="mt-1 font-mono text-[11px] text-muted-foreground">
            {{
              destination.server_side_encryption === "AES256" ? "S3 managed" : "Provider default"
            }}
          </dd>
        </div>
      </dl>
      <footer
        class="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-3"
      >
        <p class="text-[11px] text-muted-foreground">Credentials are encrypted and write-only.</p>
        <div class="flex items-center gap-2">
          <Button variant="outline" size="sm" type="button" @click="beginEditing">
            <Pencil class="size-3.5" :stroke-width="1.5" />
            Replace credentials
          </Button>
          <Button
            variant="ghost"
            size="sm"
            type="button"
            :disabled="state === 'removing'"
            @click="remove"
          >
            <Trash2 class="size-3.5 text-destructive" :stroke-width="1.5" />
            Remove
          </Button>
        </div>
      </footer>
    </div>

    <form v-else class="grid gap-4 border-t border-border px-5 py-4" @submit.prevent="save">
      <div class="grid gap-4 sm:grid-cols-2">
        <div class="grid gap-2 sm:col-span-2">
          <Label for="s3-backup-endpoint" class="text-xs font-medium">S3 endpoint</Label>
          <Input
            id="s3-backup-endpoint"
            v-model="draft.endpoint"
            class="rounded-[3px] font-mono"
            type="url"
            autocomplete="url"
            placeholder="https://s3.ap-southeast-1.amazonaws.com"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label for="s3-backup-region" class="text-xs font-medium">Region</Label>
          <Input
            id="s3-backup-region"
            v-model="draft.region"
            class="rounded-[3px] font-mono"
            autocomplete="off"
            placeholder="us-east-1"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label for="s3-backup-bucket" class="text-xs font-medium">Bucket</Label>
          <Input
            id="s3-backup-bucket"
            v-model="draft.bucket"
            class="rounded-[3px] font-mono"
            autocomplete="off"
            placeholder="ignitify-backups"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2 sm:col-span-2">
          <Label for="s3-backup-prefix" class="text-xs font-medium">Prefix</Label>
          <Input
            id="s3-backup-prefix"
            v-model="draft.prefix"
            class="rounded-[3px] font-mono"
            autocomplete="off"
            placeholder="ignitify"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label for="s3-access-key-id" class="text-xs font-medium">Access key ID</Label>
          <Input
            id="s3-access-key-id"
            v-model="draft.accessKeyId"
            class="rounded-[3px] font-mono"
            type="password"
            autocomplete="off"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label for="s3-secret-access-key" class="text-xs font-medium">Secret access key</Label>
          <Input
            id="s3-secret-access-key"
            v-model="draft.secretAccessKey"
            class="rounded-[3px] font-mono"
            type="password"
            autocomplete="off"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label for="s3-session-token" class="text-xs font-medium">Session token</Label>
          <Input
            id="s3-session-token"
            v-model="draft.sessionToken"
            class="rounded-[3px] font-mono"
            type="password"
            autocomplete="off"
            :aria-invalid="Boolean(validationError)"
          />
        </div>
        <div class="grid gap-2">
          <Label class="text-xs font-medium">Server-side encryption</Label>
          <Select v-model="draft.serverSideEncryption">
            <SelectTrigger class="rounded-[3px] font-mono text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="AES256">S3 managed (AES256)</SelectItem>
              <SelectItem value="provider-default">Provider default</SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <p v-if="validationError" class="text-[11px] text-destructive" role="alert">
        {{ validationError }}
      </p>
      <p v-else-if="requestError" class="text-[11px] text-destructive" role="alert">
        {{ requestError }}
      </p>

      <footer class="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4">
        <p class="text-[11px] text-muted-foreground">
          Credentials are never returned after saving.
        </p>
        <div class="flex items-center gap-2">
          <Button v-if="destination" variant="ghost" size="sm" type="button" @click="cancelEditing">
            <X class="size-3.5" :stroke-width="1.5" />
            Cancel
          </Button>
          <Button size="sm" type="submit" :disabled="!canSave">
            <Save class="size-3.5" :stroke-width="1.5" />
            {{ state === "saving" ? "Saving" : "Save destination" }}
          </Button>
        </div>
      </footer>
    </form>
  </section>
</template>
