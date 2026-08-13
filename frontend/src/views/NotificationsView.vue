<script setup lang="ts">
import {
  BellRing,
  Bot,
  Cable,
  CircleAlert,
  Mail,
  MessageSquareMore,
  Pencil,
  Plus,
  RefreshCw,
  Send,
  Trash2,
} from "@lucide/vue";
import { computed, onMounted, reactive, shallowRef, type Component } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import NotificationDeliveryHistory from "@/components/notifications/NotificationDeliveryHistory.vue";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
  apiCreateNotificationChannel,
  apiDeleteNotificationChannel,
  apiListNotificationChannels,
  apiListNotificationDeliveries,
  apiUpdateNotificationChannel,
  type NotificationChannel,
  type NotificationChannelInput,
  type NotificationChannelKind,
  type NotificationDelivery,
  type NotificationEventKind,
} from "@/lib/api";

const { t } = useI18n();

const channels = shallowRef<NotificationChannel[]>([]);
const deliveries = shallowRef<NotificationDelivery[]>([]);
const loading = shallowRef(true);
const deliveriesLoading = shallowRef(true);
const saving = shallowRef(false);
const removing = shallowRef(false);
const requestError = shallowRef("");
const dialogOpen = shallowRef(false);
const deleteDialogOpen = shallowRef(false);
const editingChannel = shallowRef<NotificationChannel | null>(null);
const channelPendingDeletion = shallowRef<NotificationChannel | null>(null);
const showValidation = shallowRef(false);

const kinds: { value: NotificationChannelKind; labelKey: string; icon: Component }[] = [
  { value: "telegram", labelKey: "notifications.telegram", icon: Bot },
  { value: "discord", labelKey: "notifications.discord", icon: MessageSquareMore },
  { value: "smtp", labelKey: "notifications.smtp", icon: Mail },
  { value: "resend", labelKey: "notifications.resend", icon: Send },
  { value: "webhook", labelKey: "notifications.webhook", icon: Cable },
];

const events: { value: NotificationEventKind; labelKey: string }[] = [
  { value: "deployment.queued", labelKey: "notifications.event.deploymentQueued" },
  { value: "deployment.preparing", labelKey: "notifications.event.deploymentPreparing" },
  { value: "deployment.running", labelKey: "notifications.event.deploymentRunning" },
  { value: "deployment.healthy", labelKey: "notifications.event.deploymentHealthy" },
  { value: "deployment.failed", labelKey: "notifications.event.deploymentFailed" },
  { value: "deployment.stopping", labelKey: "notifications.event.deploymentStopping" },
  { value: "deployment.stopped", labelKey: "notifications.event.deploymentStopped" },
  { value: "deployment.superseded", labelKey: "notifications.event.deploymentSuperseded" },
  { value: "backup.succeeded", labelKey: "notifications.event.backupSucceeded" },
  { value: "backup.failed", labelKey: "notifications.event.backupFailed" },
  { value: "remote_agent.offline", labelKey: "notifications.event.remoteAgentOffline" },
  {
    value: "remote_server.authentication_failed",
    labelKey: "notifications.event.remoteServerAuthenticationFailed",
  },
];

const kindIcons: Record<NotificationChannelKind, Component> = {
  telegram: Bot,
  discord: MessageSquareMore,
  smtp: Mail,
  resend: Send,
  webhook: Cable,
};

const form = reactive({
  name: "",
  kind: "telegram" as NotificationChannelKind,
  enabled: true,
  eventTypes: [
    "deployment.healthy",
    "deployment.failed",
    "backup.failed",
  ] as NotificationEventKind[],
  telegramToken: "",
  telegramChatId: "",
  discordUrl: "",
  smtpHost: "",
  smtpPort: 587,
  smtpUsername: "",
  smtpPassword: "",
  smtpFrom: "",
  smtpTo: "",
  resendApiKey: "",
  resendFrom: "",
  resendTo: "",
  webhookUrl: "",
  webhookAuthorization: "",
});

const editing = computed(() => editingChannel.value !== null);

const formError = computed(() => {
  if (!form.name.trim()) return t("notifications.validation.name");
  if (form.eventTypes.length === 0) return t("notifications.validation.events");
  if (form.kind === "telegram" && (!form.telegramToken.trim() || !form.telegramChatId.trim())) {
    return t("notifications.validation.telegram");
  }
  if (form.kind === "discord" && !form.discordUrl.trim()) {
    return t("notifications.validation.discord");
  }
  if (form.kind === "smtp") {
    if (!form.smtpHost.trim() || !form.smtpPort || !form.smtpFrom.trim() || !form.smtpTo.trim()) {
      return t("notifications.validation.smtp");
    }
    if (Boolean(form.smtpUsername.trim()) !== Boolean(form.smtpPassword.trim())) {
      return t("notifications.validation.smtpAuth");
    }
  }
  if (
    form.kind === "resend" &&
    (!form.resendApiKey.trim() || !form.resendFrom.trim() || !form.resendTo.trim())
  ) {
    return t("notifications.validation.resend");
  }
  if (form.kind === "webhook" && !/^https:\/\/.+/i.test(form.webhookUrl.trim())) {
    return t("notifications.validation.webhook");
  }
  return "";
});

function stringSummary(channel: NotificationChannel, key: string) {
  const value = channel.configuration_summary[key];
  return typeof value === "string" || typeof value === "number" ? String(value) : "";
}

function kindLabel(kind: NotificationChannelKind) {
  return t(`notifications.${kind}`);
}

function channelDestination(channel: NotificationChannel) {
  switch (channel.kind) {
    case "telegram":
      return stringSummary(channel, "chat_id");
    case "smtp":
      return `${stringSummary(channel, "host")}:${stringSummary(channel, "port")} -> ${stringSummary(channel, "to")}`;
    case "resend":
      return `${stringSummary(channel, "from")} -> ${stringSummary(channel, "to")}`;
    case "webhook":
      return stringSummary(channel, "host");
    default:
      return stringSummary(channel, "target");
  }
}

function resetForm(channel: NotificationChannel | null = null) {
  form.name = channel?.name ?? "";
  form.kind = channel?.kind ?? "telegram";
  form.enabled = channel?.enabled ?? true;
  form.eventTypes = channel?.event_types
    ? [...channel.event_types]
    : ["deployment.healthy", "deployment.failed", "backup.failed"];
  form.telegramToken = "";
  form.telegramChatId = channel ? stringSummary(channel, "chat_id") : "";
  form.discordUrl = "";
  form.smtpHost = channel ? stringSummary(channel, "host") : "";
  form.smtpPort = Number(channel ? stringSummary(channel, "port") : 587) || 587;
  form.smtpUsername = "";
  form.smtpPassword = "";
  form.smtpFrom = channel ? stringSummary(channel, "from") : "";
  form.smtpTo = channel ? stringSummary(channel, "to") : "";
  form.resendApiKey = "";
  form.resendFrom = channel ? stringSummary(channel, "from") : "";
  form.resendTo = channel ? stringSummary(channel, "to") : "";
  form.webhookUrl = "";
  form.webhookAuthorization = "";
  showValidation.value = false;
  requestError.value = "";
}

function updateDialog(open: boolean) {
  dialogOpen.value = open;
  if (!open) {
    editingChannel.value = null;
    resetForm();
  }
}

function addChannel() {
  editingChannel.value = null;
  resetForm();
  dialogOpen.value = true;
}

function editChannel(channel: NotificationChannel) {
  editingChannel.value = channel;
  resetForm(channel);
  dialogOpen.value = true;
}

function toggleEvent(event: NotificationEventKind, checked: boolean) {
  if (checked && !form.eventTypes.includes(event)) form.eventTypes.push(event);
  if (!checked) form.eventTypes = form.eventTypes.filter((value) => value !== event);
}

function configuration(): NotificationChannelInput["configuration"] {
  switch (form.kind) {
    case "telegram":
      return { bot_token: form.telegramToken.trim(), chat_id: form.telegramChatId.trim() };
    case "discord":
      return { webhook_url: form.discordUrl.trim() };
    case "smtp":
      return {
        host: form.smtpHost.trim(),
        port: form.smtpPort,
        username: form.smtpUsername.trim() || null,
        password: form.smtpPassword || null,
        from: form.smtpFrom.trim(),
        to: form.smtpTo.trim(),
        use_starttls: true,
      };
    case "resend":
      return {
        api_key: form.resendApiKey.trim(),
        from: form.resendFrom.trim(),
        to: form.resendTo.trim(),
      };
    case "webhook":
      return {
        url: form.webhookUrl.trim(),
        authorization: form.webhookAuthorization.trim() || null,
      };
  }
}

async function loadChannels(showSuccess = false) {
  loading.value = true;
  requestError.value = "";
  const result = await apiListNotificationChannels();
  loading.value = false;
  if (!result.success) {
    requestError.value = result.error ?? t("notifications.loadError");
    toast.error(t("notifications.loadError"), { description: requestError.value });
    return;
  }
  channels.value = result.data;
  if (showSuccess) toast.success(t("notifications.refreshed"));
}

async function loadDeliveries() {
  deliveriesLoading.value = true;
  const result = await apiListNotificationDeliveries();
  deliveriesLoading.value = false;
  if (result.success) deliveries.value = result.data;
}

async function saveChannel() {
  showValidation.value = true;
  if (formError.value) return;
  saving.value = true;
  requestError.value = "";
  const input: NotificationChannelInput = {
    name: form.name.trim(),
    kind: form.kind,
    enabled: form.enabled,
    event_types: [...form.eventTypes],
    configuration: configuration(),
  };
  const result = editingChannel.value
    ? await apiUpdateNotificationChannel(editingChannel.value.id, input)
    : await apiCreateNotificationChannel(input);
  saving.value = false;
  if (!result.success) {
    requestError.value = result.error ?? t("notifications.saveError");
    toast.error(t("notifications.saveError"), { description: requestError.value });
    return;
  }
  await loadChannels();
  const message = editing.value ? t("notifications.updated") : t("notifications.created");
  updateDialog(false);
  toast.success(message, { description: input.name });
}

function requestDelete(channel: NotificationChannel) {
  channelPendingDeletion.value = channel;
  deleteDialogOpen.value = true;
}

async function removeChannel() {
  const channel = channelPendingDeletion.value;
  if (!channel) return;
  removing.value = true;
  requestError.value = "";
  const result = await apiDeleteNotificationChannel(channel.id);
  removing.value = false;
  if (!result.success) {
    requestError.value = result.error ?? t("notifications.removeError");
    toast.error(t("notifications.removeError"), { description: requestError.value });
    return;
  }
  channels.value = channels.value.filter((item) => item.id !== channel.id);
  channelPendingDeletion.value = null;
  deleteDialogOpen.value = false;
  toast.success(t("notifications.removed"), { description: channel.name });
}

onMounted(async () => {
  await Promise.all([loadChannels(), loadDeliveries()]);
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">{{ t("notifications.eyebrow") }}</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">{{ t("notifications.title") }}</h1>
        <p class="mt-2 max-w-[62ch] text-sm leading-5 text-muted-foreground">
          {{ t("notifications.description") }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <Button
          variant="outline"
          size="sm"
          type="button"
          :disabled="loading"
          @click="loadChannels(true)"
        >
          <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
          {{ t("notifications.refresh") }}
        </Button>
        <Button size="sm" type="button" @click="addChannel">
          <Plus class="size-4" :stroke-width="1.5" />
          {{ t("notifications.add") }}
        </Button>
      </div>
    </header>

    <section
      class="app-surface mt-6 overflow-hidden"
      aria-labelledby="notification-channels-heading"
    >
      <header class="app-panel-header flex items-center justify-between gap-4 px-5 py-4">
        <div class="flex min-w-0 items-center gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
          >
            <BellRing class="size-4" :stroke-width="1.5" />
          </span>
          <h2 id="notification-channels-heading" class="text-base font-medium">
            {{ t("notifications.channels") }}
          </h2>
        </div>
        <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
          {{ t("notifications.channelCount", { count: channels.length }) }}
        </span>
      </header>

      <div v-if="requestError && !loading" class="border-b border-border px-5 py-3" role="alert">
        <p class="flex items-start gap-2 text-xs text-destructive">
          <CircleAlert class="mt-0.5 size-4 shrink-0" :stroke-width="1.5" />
          {{ requestError }}
        </p>
      </div>

      <div v-if="loading" class="px-5 py-10 text-sm text-muted-foreground" aria-live="polite">
        {{ t("notifications.refresh") }}
      </div>
      <div v-else-if="channels.length === 0" class="px-5 py-10">
        <BellRing class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <p class="mt-3 text-sm font-medium">{{ t("notifications.emptyTitle") }}</p>
        <p class="mt-1 max-w-[52ch] text-xs leading-5 text-muted-foreground">
          {{ t("notifications.emptyDescription") }}
        </p>
      </div>
      <div v-else class="divide-y divide-border">
        <article
          v-for="channel in channels"
          :key="channel.id"
          class="grid gap-4 px-5 py-4 md:grid-cols-[minmax(0,1fr)_auto] md:items-center"
        >
          <div class="flex min-w-0 items-start gap-3">
            <span
              class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
            >
              <component :is="kindIcons[channel.kind]" class="size-4" :stroke-width="1.5" />
            </span>
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2">
                <h3 class="truncate text-sm font-medium">{{ channel.name }}</h3>
                <span class="font-mono text-[10px] text-muted-foreground">{{
                  kindLabel(channel.kind)
                }}</span>
                <span
                  class="rounded-[3px] border px-1.5 py-0.5 font-mono text-[9px] uppercase"
                  :class="
                    channel.enabled
                      ? 'border-[var(--status-healthy)] text-[var(--status-healthy)]'
                      : 'border-border text-muted-foreground'
                  "
                >
                  {{ channel.enabled ? t("notifications.active") : t("notifications.paused") }}
                </span>
              </div>
              <p class="mt-1 truncate font-mono text-[10px] text-muted-foreground">
                {{ channelDestination(channel) }}
              </p>
              <p class="mt-2 text-[11px] text-muted-foreground">
                {{ t("notifications.eventCount", { count: channel.event_types.length }) }}
              </p>
            </div>
          </div>
          <div class="flex items-center gap-2 md:justify-end">
            <Button variant="outline" size="sm" type="button" @click="editChannel(channel)">
              <Pencil class="size-3.5" :stroke-width="1.5" />
              {{ t("notifications.edit") }}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              type="button"
              :aria-label="t('notifications.remove')"
              @click="requestDelete(channel)"
            >
              <Trash2 class="size-4 text-destructive" :stroke-width="1.5" />
            </Button>
          </div>
        </article>
      </div>
    </section>

    <NotificationDeliveryHistory
      :deliveries="deliveries"
      :loading="deliveriesLoading"
      @refresh="loadDeliveries"
    />

    <Dialog :open="dialogOpen" @update:open="updateDialog">
      <DialogContent
        class="max-h-[calc(100vh-2rem)] overflow-y-auto rounded-[10px] shadow-none sm:max-w-2xl"
      >
        <DialogHeader>
          <DialogTitle>{{
            editing ? t("notifications.editTitle") : t("notifications.createTitle")
          }}</DialogTitle>
          <DialogDescription>{{ t("notifications.dialogDescription") }}</DialogDescription>
        </DialogHeader>

        <form class="grid gap-5" @submit.prevent="saveChannel">
          <div class="grid gap-4 sm:grid-cols-2">
            <div class="grid gap-2">
              <Label for="notification-name">{{ t("notifications.name") }}</Label>
              <Input
                id="notification-name"
                v-model="form.name"
                :placeholder="t('notifications.namePlaceholder')"
                maxlength="100"
                autocomplete="off"
              />
            </div>
            <div class="grid gap-2">
              <Label for="notification-kind">{{ t("notifications.kind") }}</Label>
              <Select v-model="form.kind" :disabled="editing">
                <SelectTrigger id="notification-kind"><SelectValue /></SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="kind in kinds" :key="kind.value" :value="kind.value">
                    {{ t(kind.labelKey) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div class="flex items-center justify-between gap-4 border-y border-border py-3">
            <div>
              <Label for="notification-enabled">{{ t("notifications.enabled") }}</Label>
            </div>
            <Switch
              id="notification-enabled"
              :model-value="form.enabled"
              @update:model-value="form.enabled = $event === true"
            />
          </div>

          <fieldset class="grid gap-3">
            <legend class="text-sm font-medium">{{ t("notifications.selectedEvents") }}</legend>
            <div class="grid gap-2 sm:grid-cols-2">
              <Label
                v-for="event in events"
                :key="event.value"
                :for="`notification-event-${event.value}`"
                class="flex min-h-8 items-center gap-2 rounded-[3px] border border-border px-3 py-2 text-xs font-normal"
              >
                <Checkbox
                  :id="`notification-event-${event.value}`"
                  :checked="form.eventTypes.includes(event.value)"
                  @update:checked="toggleEvent(event.value, $event === true)"
                />
                {{ t(event.labelKey) }}
              </Label>
            </div>
          </fieldset>

          <fieldset class="grid gap-4 border-t border-border pt-5">
            <legend class="text-sm font-medium">
              {{ t("notifications.deliveryConfiguration") }}
            </legend>
            <p v-if="editing" class="-mt-2 text-[11px] leading-4 text-muted-foreground">
              {{ t("notifications.secretReplacement") }}
            </p>

            <template v-if="form.kind === 'telegram'">
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="telegram-token">{{ t("notifications.telegramToken") }}</Label
                  ><Input
                    id="telegram-token"
                    v-model="form.telegramToken"
                    type="password"
                    autocomplete="new-password"
                  />
                </div>
                <div class="grid gap-2">
                  <Label for="telegram-chat-id">{{ t("notifications.telegramChatId") }}</Label
                  ><Input
                    id="telegram-chat-id"
                    v-model="form.telegramChatId"
                    autocomplete="off"
                    placeholder="-1001234567890"
                  />
                </div>
              </div>
            </template>

            <template v-else-if="form.kind === 'discord'">
              <div class="grid gap-2">
                <Label for="discord-webhook">{{ t("notifications.discordUrl") }}</Label
                ><Input
                  id="discord-webhook"
                  v-model="form.discordUrl"
                  type="url"
                  autocomplete="off"
                  placeholder="https://discord.com/api/webhooks/..."
                />
              </div>
            </template>

            <template v-else-if="form.kind === 'smtp'">
              <div class="grid gap-4 sm:grid-cols-[minmax(0,1fr)_120px]">
                <div class="grid gap-2">
                  <Label for="smtp-host">{{ t("notifications.smtpHost") }}</Label
                  ><Input
                    id="smtp-host"
                    v-model="form.smtpHost"
                    autocomplete="off"
                    placeholder="smtp.example.com"
                  />
                </div>
                <div class="grid gap-2">
                  <Label for="smtp-port">{{ t("notifications.smtpPort") }}</Label
                  ><Input
                    id="smtp-port"
                    v-model.number="form.smtpPort"
                    type="number"
                    min="1"
                    max="65535"
                  />
                </div>
              </div>
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="smtp-username"
                    >{{ t("notifications.smtpUsername") }}
                    <span class="font-normal text-muted-foreground"
                      >({{ t("notifications.optional") }})</span
                    ></Label
                  ><Input id="smtp-username" v-model="form.smtpUsername" autocomplete="username" />
                </div>
                <div class="grid gap-2">
                  <Label for="smtp-password"
                    >{{ t("notifications.smtpPassword") }}
                    <span class="font-normal text-muted-foreground"
                      >({{ t("notifications.optional") }})</span
                    ></Label
                  ><Input
                    id="smtp-password"
                    v-model="form.smtpPassword"
                    type="password"
                    autocomplete="new-password"
                  />
                </div>
              </div>
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="smtp-from">{{ t("notifications.sender") }}</Label
                  ><Input
                    id="smtp-from"
                    v-model="form.smtpFrom"
                    type="email"
                    autocomplete="email"
                  />
                </div>
                <div class="grid gap-2">
                  <Label for="smtp-to">{{ t("notifications.recipient") }}</Label
                  ><Input id="smtp-to" v-model="form.smtpTo" type="email" autocomplete="email" />
                </div>
              </div>
            </template>

            <template v-else-if="form.kind === 'resend'">
              <div class="grid gap-2">
                <Label for="resend-api-key">{{ t("notifications.resendApiKey") }}</Label
                ><Input
                  id="resend-api-key"
                  v-model="form.resendApiKey"
                  type="password"
                  autocomplete="new-password"
                />
              </div>
              <div class="grid gap-4 sm:grid-cols-2">
                <div class="grid gap-2">
                  <Label for="resend-from">{{ t("notifications.sender") }}</Label
                  ><Input
                    id="resend-from"
                    v-model="form.resendFrom"
                    type="email"
                    autocomplete="email"
                  />
                </div>
                <div class="grid gap-2">
                  <Label for="resend-to">{{ t("notifications.recipient") }}</Label
                  ><Input
                    id="resend-to"
                    v-model="form.resendTo"
                    type="email"
                    autocomplete="email"
                  />
                </div>
              </div>
            </template>

            <template v-else>
              <div class="grid gap-2">
                <Label for="custom-webhook-url">{{ t("notifications.webhookUrl") }}</Label
                ><Input
                  id="custom-webhook-url"
                  v-model="form.webhookUrl"
                  type="url"
                  autocomplete="off"
                  placeholder="https://events.example.com/ignitify"
                />
              </div>
              <div class="grid gap-2">
                <Label for="custom-webhook-authorization"
                  >{{ t("notifications.webhookAuthorization") }}
                  <span class="font-normal text-muted-foreground"
                    >({{ t("notifications.optional") }})</span
                  ></Label
                ><Input
                  id="custom-webhook-authorization"
                  v-model="form.webhookAuthorization"
                  type="password"
                  autocomplete="new-password"
                  placeholder="Bearer ..."
                />
              </div>
            </template>
          </fieldset>

          <p v-if="showValidation && formError" class="text-xs text-destructive" role="alert">
            {{ formError }}
          </p>
          <p v-else-if="requestError" class="text-xs text-destructive" role="alert">
            {{ requestError }}
          </p>

          <DialogFooter>
            <DialogClose as-child
              ><Button variant="outline" type="button">{{
                t("notifications.cancel")
              }}</Button></DialogClose
            >
            <Button type="submit" :disabled="saving">
              {{
                saving
                  ? t("notifications.saving")
                  : editing
                    ? t("notifications.update")
                    : t("notifications.create")
              }}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="deleteDialogOpen">
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-md">
        <DialogHeader
          ><DialogTitle>{{ t("notifications.removeTitle") }}</DialogTitle
          ><DialogDescription>{{
            t("notifications.removeDescription")
          }}</DialogDescription></DialogHeader
        >
        <DialogFooter>
          <DialogClose as-child
            ><Button variant="outline" type="button">{{
              t("notifications.cancel")
            }}</Button></DialogClose
          >
          <Button variant="destructive" type="button" :disabled="removing" @click="removeChannel">{{
            t("notifications.confirmRemove")
          }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
