<script setup lang="ts">
import { Globe2, Network, Server } from "@lucide/vue";
import { computed, reactive, watch } from "vue";
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
  normalizeMonitorTarget,
  type UptimeMonitor,
  type UptimeMonitorInput,
  type UptimeMonitorKind,
} from "@/composables/useUptimeMonitors";

const props = defineProps<{
  open: boolean;
  monitor: UptimeMonitor | null;
  saving: boolean;
}>();

const emit = defineEmits<{
  "update:open": [open: boolean];
  save: [input: UptimeMonitorInput];
}>();

const form = reactive({
  name: "",
  target: "",
  kind: "http" as UptimeMonitorKind,
  intervalSeconds: 60,
  enabled: true,
});
const showValidation = computed(() => props.open && form.name.length > 0);
const targetIsValid = computed(() => Boolean(normalizeMonitorTarget(form.kind, form.target)));
const isValid = computed(() => Boolean(form.name.trim()) && targetIsValid.value);

const endpointLabel = computed(() => (form.kind === "http" ? "URL" : "Hostname and port"));
const endpointPlaceholder = computed(() =>
  form.kind === "http" ? "status.example.com/health" : "cache.example.com:6379",
);

function hydrateForm() {
  const monitor = props.monitor;
  form.name = monitor?.name ?? "";
  form.target = monitor?.target ?? "";
  form.kind = monitor?.kind ?? "http";
  form.intervalSeconds = monitor?.intervalSeconds ?? 60;
  form.enabled = monitor?.enabled ?? true;
}

function updateKind(value: unknown) {
  form.kind = value === "tcp" ? "tcp" : "http";
}

function submit() {
  if (!isValid.value) return;
  emit("save", {
    name: form.name.trim(),
    target: form.target.trim(),
    kind: form.kind,
    intervalSeconds: form.intervalSeconds,
    enabled: form.enabled,
  });
}

watch(() => [props.open, props.monitor] as const, hydrateForm, { immediate: true });
</script>

<template>
  <Dialog :open="open" @update:open="emit('update:open', $event)">
    <DialogContent class="rounded-[10px] shadow-none sm:max-w-xl">
      <DialogHeader>
        <DialogTitle class="text-base font-medium">
          {{ monitor ? "Edit monitor" : "Add monitor" }}
        </DialogTitle>
        <DialogDescription class="text-xs leading-5">
          {{
            monitor
              ? "Update the endpoint and check schedule for this monitor."
              : "Register an application, server endpoint, or external domain."
          }}
        </DialogDescription>
      </DialogHeader>

      <form class="grid gap-5" @submit.prevent="submit">
        <div class="grid gap-2">
          <Label for="uptime-monitor-name" class="text-xs font-medium">Monitor name</Label>
          <Input
            id="uptime-monitor-name"
            v-model="form.name"
            class="rounded-[3px]"
            placeholder="Customer portal"
            autocomplete="off"
            :aria-invalid="showValidation && !form.name.trim()"
          />
          <p v-if="showValidation && !form.name.trim()" class="text-[11px] text-destructive">
            A monitor name is required.
          </p>
        </div>

        <div class="grid gap-2 sm:grid-cols-2 sm:gap-4">
          <div class="grid gap-2">
            <Label for="uptime-monitor-kind" class="text-xs font-medium">Check type</Label>
            <Select :model-value="form.kind" @update:model-value="updateKind">
              <SelectTrigger id="uptime-monitor-kind" class="w-full rounded-[3px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="http">
                  <span class="flex items-center gap-2"><Globe2 class="size-4" />HTTP(s)</span>
                </SelectItem>
                <SelectItem value="tcp">
                  <span class="flex items-center gap-2"><Network class="size-4" />TCP port</span>
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="grid gap-2">
            <Label for="uptime-monitor-interval" class="text-xs font-medium">Check interval</Label>
            <Select
              :model-value="String(form.intervalSeconds)"
              @update:model-value="form.intervalSeconds = Number($event)"
            >
              <SelectTrigger id="uptime-monitor-interval" class="w-full rounded-[3px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="30">Every 30 seconds</SelectItem>
                <SelectItem value="60">Every minute</SelectItem>
                <SelectItem value="300">Every 5 minutes</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <Label for="uptime-monitor-target" class="text-xs font-medium">{{
              endpointLabel
            }}</Label>
            <span class="font-mono text-[10px] text-muted-foreground">
              {{ form.kind === "http" ? "HTTP / HTTPS" : "HOST:PORT" }}
            </span>
          </div>
          <Input
            id="uptime-monitor-target"
            v-model="form.target"
            class="rounded-[3px] font-mono text-sm"
            :placeholder="endpointPlaceholder"
            autocomplete="off"
            spellcheck="false"
            :aria-invalid="showValidation && !targetIsValid"
          />
          <p class="text-[11px] leading-4 text-muted-foreground">
            {{
              form.kind === "http"
                ? "A protocol is optional; HTTPS is used when none is provided."
                : "Use a hostname and a port number between 1 and 65535."
            }}
          </p>
          <p v-if="showValidation && !targetIsValid" class="text-[11px] text-destructive">
            Enter a valid {{ form.kind === "http" ? "HTTP(S) URL" : "hostname and port" }}.
          </p>
        </div>

        <div class="flex items-center justify-between gap-4 border-t border-border pt-4">
          <div class="flex min-w-0 items-center gap-2.5">
            <span
              class="grid size-8 shrink-0 place-items-center rounded-[4px] border border-border bg-muted text-muted-foreground"
            >
              <Server class="size-4" :stroke-width="1.5" />
            </span>
            <div>
              <p class="text-xs font-medium">Monitor enabled</p>
              <p class="mt-0.5 text-[11px] text-muted-foreground">
                Disabled monitors remain paused.
              </p>
            </div>
          </div>
          <Switch :model-value="form.enabled" @update:model-value="form.enabled = $event" />
        </div>

        <DialogFooter>
          <DialogClose as-child>
            <Button variant="outline" type="button">Cancel</Button>
          </DialogClose>
          <Button type="submit" :disabled="!isValid || saving">
            <Server class="size-4" :stroke-width="1.5" />
            {{ saving ? "Saving" : monitor ? "Save changes" : "Add monitor" }}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
