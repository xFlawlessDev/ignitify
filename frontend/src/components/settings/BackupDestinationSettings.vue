<script setup lang="ts">
import {
  CalendarClock,
  CheckCircle2,
  Cloud,
  History,
  Pencil,
  Power,
  RefreshCw,
  Save,
  Trash2,
  X,
  XCircle,
} from "@lucide/vue";
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
import { Switch } from "@/components/ui/switch";
import {
  apiDeleteBackupS3Destination,
  apiGetBackupS3Destination,
  apiListBackupS3Runs,
  apiUpdateBackupS3Controls,
  apiUpdateBackupS3Destination,
} from "@/lib/api";
import type {
  BackupS3Destination,
  BackupS3Run,
  S3ServerSideEncryption,
} from "@/lib/api/backup-destinations";

interface S3DestinationDraft {
  endpoint: string;
  region: string;
  bucket: string;
  prefix: string;
  accessKeyId: string;
  secretAccessKey: string;
  sessionToken: string;
  serverSideEncryption: S3ServerSideEncryption;
  enabled: boolean;
  schedulerEnabled: boolean;
  scheduleIntervalHours: number;
}

const destination = shallowRef<BackupS3Destination | null>(null);
const runs = shallowRef<BackupS3Run[]>([]);
const editing = shallowRef(false);
const state = shallowRef<"loading" | "idle" | "saving" | "removing" | "error">("loading");
const runsState = shallowRef<"loading" | "idle" | "error">("loading");
const controlsSaving = shallowRef(false);
const requestError = shallowRef("");
const runsError = shallowRef("");
const scheduleIntervalHours = shallowRef(24);
const draft = reactive<S3DestinationDraft>(emptyDraft());

const isDisablingExistingDestination = computed(() => !draft.enabled && destination.value !== null);

const validationError = computed(() => {
  if (isDisablingExistingDestination.value) return "";

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
  if (draft.schedulerEnabled && !validScheduleInterval(draft.scheduleIntervalHours)) {
    return "Schedule interval must be between 1 and 720 hours.";
  }
  return "";
});

const scheduleValidationError = computed(() =>
  validScheduleInterval(scheduleIntervalHours.value)
    ? ""
    : "Schedule interval must be between 1 and 720 hours.",
);

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
    enabled: false,
    schedulerEnabled: false,
    scheduleIntervalHours: 24,
  };
}

function applyDestination(value: BackupS3Destination | null) {
  destination.value = value;
  if (!value) {
    Object.assign(draft, emptyDraft());
    scheduleIntervalHours.value = 24;
    return;
  }
  const interval = value.schedule_interval_hours ?? 24;
  scheduleIntervalHours.value = interval;
  Object.assign(draft, {
    endpoint: value.endpoint,
    region: value.region,
    bucket: value.bucket,
    prefix: value.prefix,
    accessKeyId: "",
    secretAccessKey: "",
    sessionToken: "",
    serverSideEncryption: value.server_side_encryption,
    enabled: value.enabled,
    schedulerEnabled: value.schedule_interval_hours !== null,
    scheduleIntervalHours: interval,
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

async function loadRuns() {
  runsState.value = "loading";
  runsError.value = "";
  const result = await apiListBackupS3Runs();
  if (!result.success) {
    runsError.value = result.error ?? "Unable to load backup activity.";
    runsState.value = "error";
    return;
  }
  runs.value = result.data;
  runsState.value = "idle";
}

async function save() {
  if (!canSave.value) return;

  if (isDisablingExistingDestination.value && destination.value) {
    state.value = "saving";
    const updated = await updateControls(false, destination.value.schedule_interval_hours);
    state.value = updated ? "idle" : "error";
    if (updated) editing.value = false;
    return;
  }

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
    enabled: draft.enabled,
    schedule_interval_hours: draft.schedulerEnabled ? Number(draft.scheduleIntervalHours) : null,
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

async function updateControls(enabled: boolean, interval: number | null): Promise<boolean> {
  if (!destination.value || controlsSaving.value) return false;
  controlsSaving.value = true;
  requestError.value = "";
  const result = await apiUpdateBackupS3Controls({
    enabled,
    schedule_interval_hours: interval,
  });
  controlsSaving.value = false;
  if (!result.success) {
    requestError.value = result.error ?? "Unable to update backup controls.";
    return false;
  }
  applyDestination(result.data);
  return true;
}

function toggleBackup(enabled: boolean) {
  void updateControls(enabled, destination.value?.schedule_interval_hours ?? null);
}

function toggleScheduler(enabled: boolean) {
  const interval = enabled ? scheduleIntervalHours.value : null;
  void updateControls(destination.value?.enabled ?? true, interval);
}

function saveSchedule() {
  if (scheduleValidationError.value) return;
  void updateControls(destination.value?.enabled ?? true, Number(scheduleIntervalHours.value));
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

function formatDate(value: string | null) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
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

function validScheduleInterval(value: number) {
  return Number.isInteger(Number(value)) && Number(value) >= 1 && Number(value) <= 720;
}

onMounted(() => {
  void load();
  void loadRuns();
});
</script>

<template>
  <div class="grid gap-4">
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
          :class="destination?.enabled ? 'text-metric-green' : 'text-muted-foreground'"
        >
          {{
            state === "loading"
              ? "checking"
              : destination
                ? destination.enabled
                  ? "enabled"
                  : "disabled"
                : "not configured"
          }}
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
              {{ destination.bucket
              }}<span v-if="destination.prefix">/{{ destination.prefix }}</span>
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

        <div class="grid divide-y divide-border border-t border-border">
          <div class="flex items-start justify-between gap-4 px-5 py-4">
            <div class="flex min-w-0 items-start gap-3">
              <Power class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
              <div>
                <Label for="backup-enabled" class="text-xs font-medium">S3 backup</Label>
                <p class="mt-1 max-w-[54ch] text-[11px] leading-4 text-muted-foreground">
                  Disable uploads without removing the saved destination or credentials.
                </p>
              </div>
            </div>
            <Switch
              id="backup-enabled"
              :disabled="controlsSaving"
              :model-value="destination.enabled"
              aria-label="Enable S3 backup"
              @update:model-value="toggleBackup"
            />
          </div>

          <div class="grid gap-4 px-5 py-4">
            <div class="flex items-start justify-between gap-4">
              <div class="flex min-w-0 items-start gap-3">
                <CalendarClock
                  class="mt-0.5 size-4 shrink-0 text-muted-foreground"
                  :stroke-width="1.5"
                />
                <div>
                  <Label for="backup-schedule-enabled" class="text-xs font-medium">
                    Scheduled backup
                  </Label>
                  <p class="mt-1 max-w-[54ch] text-[11px] leading-4 text-muted-foreground">
                    Run an S3 backup on an interval while S3 backup is enabled.
                  </p>
                </div>
              </div>
              <Switch
                id="backup-schedule-enabled"
                :disabled="controlsSaving"
                :model-value="destination.schedule_interval_hours !== null"
                aria-label="Enable scheduled backups"
                @update:model-value="toggleScheduler"
              />
            </div>

            <div
              v-if="destination.schedule_interval_hours !== null"
              class="flex flex-wrap items-end gap-3"
            >
              <div class="grid w-full max-w-52 gap-2">
                <Label for="backup-schedule-interval" class="text-xs font-medium">
                  Interval (hours)
                </Label>
                <Input
                  id="backup-schedule-interval"
                  v-model.number="scheduleIntervalHours"
                  class="rounded-[3px] font-mono"
                  type="number"
                  min="1"
                  max="720"
                  step="1"
                  inputmode="numeric"
                  :aria-invalid="Boolean(scheduleValidationError)"
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                type="button"
                :disabled="controlsSaving || Boolean(scheduleValidationError)"
                @click="saveSchedule"
              >
                <Save class="size-3.5" :stroke-width="1.5" />
                Save schedule
              </Button>
              <p
                v-if="scheduleValidationError"
                class="w-full text-[11px] text-destructive"
                role="alert"
              >
                {{ scheduleValidationError }}
              </p>
            </div>
          </div>
        </div>

        <p
          v-if="requestError"
          class="border-t border-border px-5 py-3 text-[11px] text-destructive"
          role="alert"
        >
          {{ requestError }}
        </p>

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
              <SelectTrigger class="rounded-[3px] font-mono text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="AES256">S3 managed (AES256)</SelectItem>
                <SelectItem value="provider-default">Provider default</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div class="grid gap-4 border-t border-border pt-4">
          <div class="flex items-start justify-between gap-4">
            <div>
              <Label for="draft-backup-enabled" class="text-xs font-medium">Enable S3 backup</Label>
              <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
                Keep the destination saved but inactive when disabled.
              </p>
            </div>
            <Switch
              id="draft-backup-enabled"
              v-model="draft.enabled"
              aria-label="Enable S3 backup"
            />
          </div>
          <div class="flex items-start justify-between gap-4">
            <div>
              <Label for="draft-backup-scheduler" class="text-xs font-medium"
                >Scheduled backup</Label
              >
              <p class="mt-1 text-[11px] leading-4 text-muted-foreground">
                Optionally run on a fixed interval.
              </p>
            </div>
            <Switch
              id="draft-backup-scheduler"
              v-model="draft.schedulerEnabled"
              aria-label="Enable scheduled backups"
            />
          </div>
          <div v-if="draft.schedulerEnabled" class="grid max-w-52 gap-2">
            <Label for="draft-backup-schedule-interval" class="text-xs font-medium"
              >Interval (hours)</Label
            >
            <Input
              id="draft-backup-schedule-interval"
              v-model.number="draft.scheduleIntervalHours"
              class="rounded-[3px] font-mono"
              type="number"
              min="1"
              max="720"
              step="1"
              inputmode="numeric"
              :aria-invalid="Boolean(validationError)"
            />
          </div>
        </div>

        <p v-if="validationError" class="text-[11px] text-destructive" role="alert">
          {{ validationError }}
        </p>
        <p v-else-if="requestError" class="text-[11px] text-destructive" role="alert">
          {{ requestError }}
        </p>

        <footer
          class="flex flex-wrap items-center justify-between gap-3 border-t border-border pt-4"
        >
          <p class="text-[11px] text-muted-foreground">
            Credentials are never returned after saving.
          </p>
          <div class="flex items-center gap-2">
            <Button
              v-if="destination"
              variant="ghost"
              size="sm"
              type="button"
              @click="cancelEditing"
            >
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

    <section class="app-surface" aria-labelledby="backup-history-heading">
      <header class="app-panel-header flex items-start justify-between gap-4 px-5 py-4">
        <div class="flex items-start gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
          >
            <History class="size-4" :stroke-width="1.5" />
          </span>
          <div>
            <p class="ui-label">Backup activity</p>
            <h2 id="backup-history-heading" class="mt-1.5 text-base font-medium">Run history</h2>
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          type="button"
          :disabled="runsState === 'loading'"
          aria-label="Refresh backup history"
          title="Refresh backup history"
          @click="loadRuns"
        >
          <RefreshCw
            class="size-4"
            :class="runsState === 'loading' ? 'animate-spin' : ''"
            :stroke-width="1.5"
          />
        </Button>
      </header>

      <p
        v-if="runsError"
        class="border-t border-border px-5 py-3 text-[11px] text-destructive"
        role="alert"
      >
        {{ runsError }}
      </p>
      <div
        v-else-if="runsState === 'loading'"
        class="border-t border-border px-5 py-4 text-xs text-muted-foreground"
      >
        Loading backup activity
      </div>
      <div v-else-if="!runs.length" class="border-t border-border px-5 py-7">
        <p class="text-sm font-medium">No backup runs yet</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Manual and scheduled backup outcomes will appear here.
        </p>
      </div>
      <div v-else class="overflow-x-auto border-t border-border">
        <table class="w-full min-w-[580px] text-left text-xs">
          <thead class="border-b border-border text-[10px] text-muted-foreground">
            <tr>
              <th class="px-5 py-3 font-medium">Status</th>
              <th class="px-4 py-3 font-medium">Trigger</th>
              <th class="px-4 py-3 font-medium">Started</th>
              <th class="px-4 py-3 font-medium">Completed</th>
              <th class="px-4 py-3 font-medium">Detail</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-border">
            <tr v-for="run in runs" :key="run.id">
              <td class="px-5 py-3">
                <span
                  class="inline-flex items-center gap-1.5 font-mono text-[10px]"
                  :class="
                    run.status === 'succeeded'
                      ? 'text-metric-green'
                      : run.status === 'failed'
                        ? 'text-destructive'
                        : 'text-muted-foreground'
                  "
                >
                  <CheckCircle2
                    v-if="run.status === 'succeeded'"
                    class="size-3.5"
                    :stroke-width="1.5"
                  />
                  <XCircle
                    v-else-if="run.status === 'failed'"
                    class="size-3.5"
                    :stroke-width="1.5"
                  />
                  <RefreshCw v-else class="size-3.5 animate-spin" :stroke-width="1.5" />
                  {{ run.status }}
                </span>
              </td>
              <td class="px-4 py-3 font-mono text-[11px] text-muted-foreground">
                {{ run.trigger }}
              </td>
              <td class="px-4 py-3 whitespace-nowrap text-[11px] text-muted-foreground">
                {{ formatDate(run.started_at) }}
              </td>
              <td class="px-4 py-3 whitespace-nowrap text-[11px] text-muted-foreground">
                {{ formatDate(run.completed_at) }}
              </td>
              <td class="max-w-64 px-4 py-3 text-[11px] text-muted-foreground">
                {{ run.message ?? "-" }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </section>
  </div>
</template>
