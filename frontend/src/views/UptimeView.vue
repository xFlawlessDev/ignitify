<script setup lang="ts">
import {
  CircleAlert,
  CirclePause,
  Clock3,
  Ellipsis,
  ExternalLink,
  Globe2,
  Plus,
  RefreshCw,
  Server,
  Trash2,
} from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import UptimeMonitorDialog from "@/components/uptime/UptimeMonitorDialog.vue";
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import {
  useUptimeMonitors,
  type UptimeCheckState,
  type UptimeMonitor,
  type UptimeMonitorInput,
  type UptimeMonitorStatus,
} from "@/composables/useUptimeMonitors";

type DisplayStatus = UptimeMonitorStatus | "paused";
type StatusFilter = "all" | DisplayStatus;

const {
  monitors,
  loading,
  refreshing,
  saving,
  error,
  addMonitor,
  updateMonitor,
  removeMonitor,
  reloadMonitors,
} = useUptimeMonitors();
const search = shallowRef("");
const statusFilter = shallowRef<StatusFilter>("all");
const dialogOpen = shallowRef(false);
const editingMonitor = shallowRef<UptimeMonitor | null>(null);
const monitorPendingRemoval = shallowRef<UptimeMonitor | null>(null);
const lastUpdated = shallowRef(new Date());
let refreshTimer: number | undefined;

const filterOptions: { label: string; value: StatusFilter }[] = [
  { label: "All", value: "all" },
  { label: "Operational", value: "up" },
  { label: "Incident", value: "down" },
  { label: "Pending", value: "pending" },
  { label: "Paused", value: "paused" },
];

function displayStatus(monitor: UptimeMonitor): DisplayStatus {
  return monitor.enabled ? monitor.status : "paused";
}

const statusSummary = computed(() => ({
  total: monitors.value.length,
  up: monitors.value.filter((monitor) => displayStatus(monitor) === "up").length,
  down: monitors.value.filter((monitor) => displayStatus(monitor) === "down").length,
  pending: monitors.value.filter((monitor) => displayStatus(monitor) === "pending").length,
  paused: monitors.value.filter((monitor) => displayStatus(monitor) === "paused").length,
}));

const visibleMonitors = computed(() => {
  const query = search.value.trim().toLowerCase();
  return monitors.value.filter((monitor) => {
    const matchesStatus =
      statusFilter.value === "all" || displayStatus(monitor) === statusFilter.value;
    const matchesQuery =
      !query ||
      monitor.name.toLowerCase().includes(query) ||
      monitor.target.toLowerCase().includes(query);
    return matchesStatus && matchesQuery;
  });
});

const lastUpdatedLabel = computed(() =>
  new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  }).format(lastUpdated.value),
);

function monitorHost(monitor: UptimeMonitor): string {
  if (monitor.kind === "tcp") return monitor.target;
  try {
    return new URL(monitor.target).host;
  } catch {
    return monitor.target;
  }
}

function monitorPath(monitor: UptimeMonitor): string {
  if (monitor.kind === "tcp") return "TCP endpoint";
  try {
    const url = new URL(monitor.target);
    return `${url.protocol.replace(":", "").toUpperCase()} ${url.pathname}${url.search}`;
  } catch {
    return monitor.target;
  }
}

function statusLabel(status: DisplayStatus): string {
  if (status === "up") return "Operational";
  if (status === "down") return "Incident";
  if (status === "paused") return "Paused";
  return "Pending";
}

function statusDot(status: DisplayStatus): "healthy" | "failed" | undefined {
  if (status === "up") return "healthy";
  if (status === "down") return "failed";
  return undefined;
}

function statusTextClass(status: DisplayStatus): string {
  if (status === "up") return "text-[var(--status-healthy)]";
  if (status === "down") return "text-destructive";
  if (status === "pending") return "text-signal-orange";
  return "text-muted-foreground";
}

function historyClass(state: UptimeCheckState): string {
  if (state === "up") return "bg-[var(--status-healthy)]";
  if (state === "down") return "bg-destructive";
  if (state === "pending") return "bg-muted-foreground/45";
  return "bg-border";
}

function formatInterval(intervalSeconds: number): string {
  if (intervalSeconds < 60) return `${intervalSeconds}s`;
  if (intervalSeconds % 60 === 0) return `${intervalSeconds / 60}m`;
  return `${intervalSeconds}s`;
}

function formatLastChecked(value: string | null): string {
  if (!value) return "No completed check";
  return new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).format(new Date(value));
}

function openAddDialog() {
  editingMonitor.value = null;
  dialogOpen.value = true;
}

function openEditDialog(monitor: UptimeMonitor) {
  editingMonitor.value = monitor;
  dialogOpen.value = true;
}

function updateDialog(open: boolean) {
  dialogOpen.value = open;
  if (!open) editingMonitor.value = null;
}

async function saveMonitor(input: UptimeMonitorInput) {
  const result = editingMonitor.value
    ? await updateMonitor(editingMonitor.value.id, input)
    : await addMonitor(input);
  if (!result) return;
  lastUpdated.value = new Date();
  updateDialog(false);
}

function confirmRemove(monitor: UptimeMonitor) {
  monitorPendingRemoval.value = monitor;
}

async function removeSelectedMonitor() {
  if (!monitorPendingRemoval.value) return;
  const removed = await removeMonitor(monitorPendingRemoval.value.id);
  if (!removed) return;
  monitorPendingRemoval.value = null;
  lastUpdated.value = new Date();
}

async function reloadConfiguration() {
  await reloadMonitors();
  lastUpdated.value = new Date();
}

onMounted(() => {
  void reloadConfiguration();
  refreshTimer = window.setInterval(() => void reloadConfiguration(), 30_000);
});

onUnmounted(() => {
  if (refreshTimer !== undefined) window.clearInterval(refreshTimer);
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Availability</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Uptime</h1>
        <p class="mt-2 max-w-[62ch] text-sm leading-5 text-muted-foreground">
          Endpoint status across applications, servers, and external domains.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <Button
          class="min-w-0 flex-1 sm:flex-none"
          size="sm"
          variant="outline"
          type="button"
          :disabled="loading || refreshing || saving"
          title="Reload monitor configuration"
          @click="reloadConfiguration"
        >
          <RefreshCw
            class="size-4"
            :class="loading || refreshing ? 'animate-spin' : ''"
            :stroke-width="1.5"
          />
          Reload
        </Button>
        <Button
          class="min-w-0 flex-1 sm:flex-none"
          size="sm"
          type="button"
          :disabled="saving"
          @click="openAddDialog"
        >
          <Plus class="size-4" :stroke-width="1.5" />
          Add monitor
        </Button>
      </div>
    </header>

    <section
      v-if="error"
      class="mt-4 flex items-center gap-2.5 rounded-[10px] border border-destructive/40 bg-card px-4 py-3 text-xs text-destructive"
      role="alert"
    >
      <CircleAlert class="size-4 shrink-0" :stroke-width="1.5" />
      <p>{{ error }}</p>
      <Button
        class="ml-auto shrink-0"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="reloadConfiguration"
      >
        Retry
      </Button>
    </section>

    <section
      class="mt-6 app-surface grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
      aria-label="Monitor availability summary"
    >
      <div class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
        <div class="flex items-center justify-between gap-4">
          <p class="ui-label">Monitors</p>
          <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
        </div>
        <p class="font-mono text-3xl leading-none">{{ statusSummary.total }}</p>
      </div>
      <div class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
        <div class="flex items-center justify-between gap-4">
          <p class="ui-label">Operational</p>
          <span class="status-dot" data-status="healthy" aria-hidden="true" />
        </div>
        <p class="font-mono text-3xl leading-none text-[var(--status-healthy)]">
          {{ statusSummary.up }}
        </p>
      </div>
      <div class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
        <div class="flex items-center justify-between gap-4">
          <p class="ui-label">Incident</p>
          <CircleAlert class="size-4 text-destructive" :stroke-width="1.5" />
        </div>
        <p class="font-mono text-3xl leading-none text-destructive">{{ statusSummary.down }}</p>
      </div>
      <div class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
        <div class="flex items-center justify-between gap-4">
          <p class="ui-label">Pending</p>
          <Clock3 class="size-4 text-signal-orange" :stroke-width="1.5" />
        </div>
        <p class="font-mono text-3xl leading-none text-signal-orange">
          {{ statusSummary.pending }}
        </p>
        <p v-if="statusSummary.paused" class="mt-1 font-mono text-[10px] text-muted-foreground">
          {{ statusSummary.paused }} PAUSED
        </p>
      </div>
    </section>

    <section class="mt-6" aria-labelledby="monitor-inventory-heading">
      <div class="flex flex-wrap items-end justify-between gap-4 border-b border-border pb-4">
        <div>
          <p class="ui-label">Endpoint inventory</p>
          <h2 id="monitor-inventory-heading" class="mt-2 text-base font-medium">Monitors</h2>
        </div>
        <p class="font-mono text-[10px] text-muted-foreground" aria-live="polite">
          CONFIGURATION UPDATED {{ lastUpdatedLabel }}
        </p>
      </div>

      <div class="mt-4 grid gap-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
        <Input
          v-model="search"
          class="h-9 min-w-0 rounded-[3px]"
          placeholder="Filter by name or endpoint"
          aria-label="Filter monitors"
        />
        <div
          class="flex min-w-0 overflow-x-auto rounded-[4px] border border-border bg-card p-0.5"
          role="group"
          aria-label="Monitor status filter"
        >
          <Button
            v-for="option in filterOptions"
            :key="option.value"
            variant="ghost"
            type="button"
            class="h-8 shrink-0 px-2.5 font-mono text-[10px] text-muted-foreground hover:bg-muted hover:text-foreground"
            :class="statusFilter === option.value ? 'bg-muted text-foreground' : ''"
            :aria-pressed="statusFilter === option.value"
            @click="statusFilter = option.value"
          >
            {{ option.label }}
          </Button>
        </div>
      </div>

      <div
        v-if="loading"
        class="mt-4 grid gap-3"
        role="status"
        aria-label="Loading uptime monitors"
      >
        <article v-for="index in 3" :key="index" class="app-surface grid gap-5 px-5 py-5">
          <div class="h-3 w-36 animate-pulse rounded-[2px] bg-muted" />
          <div class="h-2.5 w-64 max-w-full animate-pulse rounded-[2px] bg-muted" />
          <div class="h-5 w-full animate-pulse rounded-[2px] bg-muted" />
        </article>
      </div>
      <div v-else-if="visibleMonitors.length" class="mt-4 grid gap-3">
        <article v-for="monitor in visibleMonitors" :key="monitor.id" class="app-surface min-w-0">
          <header
            class="app-panel-header grid gap-3 px-4 py-3.5 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:px-5"
          >
            <div class="flex min-w-0 items-center gap-3">
              <span
                class="grid size-8 shrink-0 place-items-center rounded-[4px] border border-border bg-card text-muted-foreground"
              >
                <Globe2 v-if="monitor.kind === 'http'" class="size-4" :stroke-width="1.5" />
                <Server v-else class="size-4" :stroke-width="1.5" />
              </span>
              <div class="min-w-0">
                <h3 class="truncate text-sm font-medium">{{ monitor.name }}</h3>
                <p class="mt-0.5 truncate font-mono text-[10px] text-muted-foreground">
                  {{ monitorPath(monitor) }}
                </p>
              </div>
            </div>
            <div class="flex items-center justify-between gap-3 sm:justify-end">
              <span
                class="flex items-center gap-2 text-xs"
                :class="statusTextClass(displayStatus(monitor))"
              >
                <span
                  class="status-dot"
                  :data-status="statusDot(displayStatus(monitor))"
                  aria-hidden="true"
                />
                {{ statusLabel(displayStatus(monitor)) }}
              </span>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    size="icon-sm"
                    variant="ghost"
                    type="button"
                    :aria-label="`Actions for ${monitor.name}`"
                    title="Monitor actions"
                  >
                    <Ellipsis class="size-4" :stroke-width="1.5" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" class="w-40">
                  <DropdownMenuItem @select="openEditDialog(monitor)"
                    >Edit monitor</DropdownMenuItem
                  >
                  <DropdownMenuItem variant="destructive" @select="confirmRemove(monitor)">
                    <Trash2 class="size-4" :stroke-width="1.5" />
                    Remove monitor
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </div>
          </header>

          <div
            class="grid gap-5 px-4 py-4 sm:px-5 lg:grid-cols-[minmax(0,1fr)_260px] lg:items-center"
          >
            <div class="min-w-0">
              <div class="flex min-w-0 items-center gap-2">
                <a
                  v-if="monitor.kind === 'http'"
                  class="min-w-0 truncate font-mono text-xs text-foreground underline decoration-border underline-offset-4 transition-colors hover:text-signal-orange"
                  :href="monitor.target"
                  target="_blank"
                  rel="noreferrer"
                  :title="monitor.target"
                >
                  {{ monitorHost(monitor) }}
                </a>
                <p v-else class="min-w-0 truncate font-mono text-xs" :title="monitor.target">
                  {{ monitorHost(monitor) }}
                </p>
                <ExternalLink
                  v-if="monitor.kind === 'http'"
                  class="size-3.5 shrink-0 text-muted-foreground"
                  :stroke-width="1.5"
                />
              </div>
              <p v-if="monitor.lastError" class="mt-2 truncate text-[11px] text-destructive">
                {{ monitor.lastError }}
              </p>
              <div class="mt-4 grid grid-cols-3 divide-x divide-border border-y border-border">
                <div class="min-w-0 py-2 pr-3">
                  <p class="font-mono text-[10px] text-muted-foreground uppercase">Latency</p>
                  <p class="mt-1 truncate font-mono text-xs">
                    {{ monitor.latencyMs !== null ? `${monitor.latencyMs} ms` : "—" }}
                  </p>
                </div>
                <div class="min-w-0 px-3 py-2">
                  <p class="font-mono text-[10px] text-muted-foreground uppercase">Interval</p>
                  <p class="mt-1 font-mono text-xs">
                    {{ formatInterval(monitor.intervalSeconds) }}
                  </p>
                </div>
                <div class="min-w-0 py-2 pl-3">
                  <p class="font-mono text-[10px] text-muted-foreground uppercase">Last check</p>
                  <p
                    class="mt-1 truncate font-mono text-xs"
                    :title="formatLastChecked(monitor.lastCheckedAt)"
                  >
                    {{ formatLastChecked(monitor.lastCheckedAt) }}
                  </p>
                </div>
              </div>
            </div>

            <div class="min-w-0" :aria-label="`Recent check history for ${monitor.name}`">
              <div class="flex items-center justify-between gap-3">
                <p class="font-mono text-[10px] text-muted-foreground uppercase">30 checks</p>
                <p class="font-mono text-[10px] text-muted-foreground">NO DATA</p>
              </div>
              <div class="mt-2 grid grid-cols-[repeat(30,minmax(0,1fr))] gap-1" aria-hidden="true">
                <span
                  v-for="(state, index) in monitor.history"
                  :key="index"
                  class="h-5 min-w-0 rounded-[2px]"
                  :class="historyClass(state)"
                />
              </div>
            </div>
          </div>
        </article>
      </div>

      <section
        v-else-if="monitors.length"
        class="mt-4 app-surface grid min-h-48 place-items-center px-5 py-8 text-center"
        aria-live="polite"
      >
        <div>
          <CirclePause class="mx-auto size-5 text-muted-foreground" :stroke-width="1.5" />
          <p class="mt-3 text-sm font-medium">No monitors match this view</p>
          <Button
            class="mt-4"
            size="sm"
            variant="outline"
            type="button"
            @click="
              statusFilter = 'all';
              search = '';
            "
          >
            Clear filters
          </Button>
        </div>
      </section>

      <section
        v-else
        class="mt-4 app-surface grid min-h-64 place-items-center px-5 py-10 text-center"
      >
        <div class="max-w-sm">
          <span
            class="mx-auto grid size-10 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
          >
            <Server class="size-5" :stroke-width="1.5" />
          </span>
          <h2 class="mt-4 text-base font-medium">No monitors configured</h2>
          <p class="mt-2 text-xs leading-5 text-muted-foreground">
            Add a public application URL, custom domain, or TCP endpoint.
          </p>
          <Button class="mt-5" size="sm" type="button" @click="openAddDialog">
            <Plus class="size-4" :stroke-width="1.5" />
            Add monitor
          </Button>
        </div>
      </section>
    </section>

    <UptimeMonitorDialog
      :open="dialogOpen"
      :monitor="editingMonitor"
      :saving="saving"
      @update:open="updateDialog"
      @save="saveMonitor"
    />

    <Dialog
      :open="Boolean(monitorPendingRemoval)"
      @update:open="(open) => !open && (monitorPendingRemoval = null)"
    >
      <DialogContent class="rounded-[10px] shadow-none sm:max-w-md">
        <DialogHeader>
          <DialogTitle class="text-base font-medium">Remove monitor</DialogTitle>
          <DialogDescription class="text-xs leading-5">
            {{ monitorPendingRemoval?.name }} and its recorded check history will be removed.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <DialogClose as-child
            ><Button variant="outline" type="button">Cancel</Button></DialogClose
          >
          <Button
            variant="destructive"
            type="button"
            :disabled="saving"
            @click="removeSelectedMonitor"
          >
            <Trash2 class="size-4" :stroke-width="1.5" />
            Remove monitor
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
