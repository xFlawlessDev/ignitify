<script setup lang="ts">
import {
  Check,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Container,
  Copy,
  Cpu,
  FileText,
  FolderOpen,
  MoreHorizontal,
  Network,
  Layers3,
  RefreshCw,
  Server,
  Settings2,
  Terminal,
  Trash2,
  Upload,
} from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef, watch } from "vue";
import { PopoverClose } from "reka-ui";
import ContainerActionDialog from "@/components/runtime/ContainerActionDialog.vue";
import type { ContainerActionKey } from "@/components/runtime/container-actions";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useRuntimeContainers } from "@/composables/useRuntimeContainers";
import { useRuntimeStatus } from "@/composables/useRuntimeStatus";
import type { RuntimeContainer, RuntimePort } from "@/lib/types";

const {
  data: runtime,
  error: runtimeError,
  load: loadRuntime,
  loading: runtimeLoading,
} = useRuntimeStatus();
const {
  data: containers,
  error: inventoryError,
  load: loadContainers,
  loading: inventoryLoading,
} = useRuntimeContainers();
const metrics = computed(() => runtime.value?.metrics ?? null);
const error = computed(() => runtimeError.value ?? inventoryError.value);
const loading = computed(() => runtimeLoading.value || inventoryLoading.value);
const CONTAINERS_PER_PAGE = 10;
const currentPage = shallowRef(1);
const containerCount = computed(() => containers.value?.length ?? 0);
const pageCount = computed(() =>
  Math.max(1, Math.ceil(containerCount.value / CONTAINERS_PER_PAGE)),
);
const visibleContainers = computed(() => {
  const start = (currentPage.value - 1) * CONTAINERS_PER_PAGE;
  return (containers.value ?? []).slice(start, start + CONTAINERS_PER_PAGE);
});
const firstVisibleContainer = computed(() =>
  containerCount.value === 0 ? 0 : (currentPage.value - 1) * CONTAINERS_PER_PAGE + 1,
);
const lastVisibleContainer = computed(() =>
  Math.min(currentPage.value * CONTAINERS_PER_PAGE, containerCount.value),
);

const REFRESH_INTERVAL_MS = 3_000;
let refreshTimer: number | undefined;
let copyTimer: number | undefined;

const containerActions = [
  {
    key: "logs",
    label: "View Logs",
    description: "Inspect the latest output emitted by this container.",
    icon: FileText,
  },
  {
    key: "config",
    label: "View Config",
    description: "Review the runtime metadata currently known for this container.",
    icon: Settings2,
  },
  {
    key: "mounts",
    label: "View Mounts",
    description: "Inspect filesystems mounted into this container.",
    icon: FolderOpen,
  },
  {
    key: "networks",
    label: "View Networks",
    description: "Inspect network attachments and published ports.",
    icon: Network,
  },
  {
    key: "terminal",
    label: "Terminal",
    description: "Open an interactive shell for this container.",
    icon: Terminal,
  },
  {
    key: "upload",
    label: "Upload File",
    description: "Choose a file to upload into this container.",
    icon: Upload,
  },
] as const;

const copiedContainerId = shallowRef<string | null>(null);
const selectedContainer = shallowRef<RuntimeContainer | null>(null);
const activeAction = shallowRef<ContainerActionKey | null>(null);
const actionDialogOpen = shallowRef(false);
const removeDialogOpen = shallowRef(false);

function formatBytes(value: number | null | undefined) {
  if (!value) return "Unavailable";
  if (value >= 1024 ** 3) return `${(value / 1024 ** 3).toFixed(1)} GiB`;
  return `${(value / 1024 ** 2).toFixed(0)} MiB`;
}

function formatCpuLimit(value: number | null) {
  if (!value) return "No limit";
  return `${(value / 1_000_000_000).toFixed(1)} CPU`;
}

function statusClass(status: string) {
  if (status.startsWith("Up")) return "text-[var(--status-healthy)]";
  if (status.startsWith("Restarting")) return "text-[var(--status-live)]";
  return "text-muted-foreground";
}

function formatPort(port: RuntimePort) {
  const container = `${port.container_port}/${port.protocol}`;
  if (port.host_port === null) return container;
  return `${port.host_ip ?? "0.0.0.0"}:${port.host_port} → ${container}`;
}

function goToPage(page: number) {
  currentPage.value = Math.min(Math.max(page, 1), pageCount.value);
}

function goToPreviousPage() {
  goToPage(currentPage.value - 1);
}

function goToNextPage() {
  goToPage(currentPage.value + 1);
}

async function copyContainerId(container: RuntimeContainer) {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(container.id);
    } else {
      const input = document.createElement("textarea");
      input.value = container.id;
      input.setAttribute("readonly", "true");
      input.style.position = "fixed";
      input.style.opacity = "0";
      document.body.append(input);
      input.select();
      document.execCommand("copy");
      input.remove();
    }
  } catch {
    return;
  }

  copiedContainerId.value = container.id;
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  copyTimer = window.setTimeout(() => {
    copiedContainerId.value = null;
    copyTimer = undefined;
  }, 1_600);
}

function openContainerAction(container: RuntimeContainer, action: ContainerActionKey) {
  selectedContainer.value = container;
  activeAction.value = action;
  if (action === "remove") {
    removeDialogOpen.value = true;
    return;
  }
  actionDialogOpen.value = true;
}

watch(
  containerCount,
  () => {
    if (currentPage.value > pageCount.value) currentPage.value = pageCount.value;
  },
  { immediate: true },
);

function scheduleRefresh(delay = REFRESH_INTERVAL_MS) {
  if (document.visibilityState === "hidden") return;
  refreshTimer = window.setTimeout(() => {
    void refresh();
  }, delay);
}

function handleVisibilityChange() {
  if (document.visibilityState === "hidden") {
    if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
    refreshTimer = undefined;
    return;
  }
  if (refreshTimer === undefined) void refresh();
}

async function refresh() {
  if (loading.value) return;
  if (refreshTimer !== undefined) {
    window.clearTimeout(refreshTimer);
    refreshTimer = undefined;
  }
  const startedAt = Date.now();
  await Promise.all([loadRuntime(), loadContainers()]);
  scheduleRefresh(Math.max(0, REFRESH_INTERVAL_MS - (Date.now() - startedAt)));
}

async function handleContainerRemoved() {
  selectedContainer.value = null;
  activeAction.value = null;
  await refresh();
}

onMounted(() => {
  document.addEventListener("visibilitychange", handleVisibilityChange);
  void refresh();
});

onUnmounted(() => {
  if (refreshTimer !== undefined) window.clearTimeout(refreshTimer);
  if (copyTimer !== undefined) window.clearTimeout(copyTimer);
  document.removeEventListener("visibilitychange", handleVisibilityChange);
});
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Docker</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Containers</h1>
        <p class="mt-2 text-sm text-muted-foreground">
          Runtime health and aggregate container capacity for this host.
        </p>
      </div>
      <Button
        class="w-full shrink-0 sm:w-auto"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="refresh"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Refresh
      </Button>
    </header>

    <section
      class="mt-6 app-surface grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0 lg:grid-cols-4"
      aria-label="Docker capacity"
    >
      <template v-if="loading && !runtime">
        <div v-for="index in 4" :key="index" class="min-h-36 bg-background p-5">
          <Skeleton class="h-3 w-20" />
          <Skeleton class="mt-12 h-9 w-16" />
        </div>
      </template>
      <template v-else>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Containers</p>
            <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? metrics.containers : "—" }}
          </p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Running</p>
            <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? metrics.containers_running : "—" }}
          </p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Images</p>
            <Layers3 class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">{{ metrics ? metrics.images : "—" }}</p>
        </section>
        <section class="grid min-h-32 content-between bg-background p-4 sm:min-h-36 sm:p-5">
          <div class="flex items-center justify-between gap-4">
            <p class="ui-label">Memory</p>
            <Cpu class="size-4 text-muted-foreground" :stroke-width="1.5" />
          </div>
          <p class="text-3xl leading-none tracking-normal">
            {{ metrics ? formatBytes(metrics.memory_bytes) : "—" }}
          </p>
        </section>
      </template>
    </section>

    <section
      v-if="error"
      class="mt-4 rounded-[10px] flex items-start justify-between gap-4 border border-destructive/40 bg-card px-5 py-4 max-[640px]:flex-col"
      role="alert"
    >
      <div class="flex items-start gap-2 text-sm text-destructive">
        <CircleAlert class="mt-0.5 size-4 shrink-0" :stroke-width="1.5" />
        <p>{{ error }}</p>
      </div>
      <Button
        class="shrink-0 max-[640px]:w-full"
        size="sm"
        variant="outline"
        :disabled="loading"
        @click="refresh"
      >
        <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        Retry
      </Button>
    </section>

    <section class="mt-6 min-w-0 gap-4">
      <section class="min-w-0 app-surface">
        <div class="app-panel-header px-5 py-4">
          <p class="ui-label">Container inventory</p>
          <h2 class="mt-2 text-base font-medium">Per-container monitoring</h2>
        </div>
        <div
          v-if="inventoryLoading && containers === null"
          class="grid gap-4 px-5 py-5"
          role="status"
          aria-label="Loading container inventory"
        >
          <div v-for="index in 6" :key="index" class="grid grid-cols-2 gap-4 sm:grid-cols-4">
            <Skeleton class="h-3 w-32 max-w-full" />
            <Skeleton class="h-3 w-40 max-w-full" />
            <Skeleton class="h-3 w-24 max-w-full" />
            <Skeleton class="h-3 w-16 max-w-full" />
          </div>
        </div>
        <div v-else-if="containers === null" class="px-5 py-8">
          <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <p class="mt-4 text-sm font-medium">Docker inventory unavailable</p>
          <p class="mt-1 max-w-xl text-xs leading-5 text-muted-foreground">
            Docker did not return container details for this host. Check runtime connectivity, then
            refresh.
          </p>
        </div>
        <Table v-else-if="containers.length">
          <TableHeader>
            <TableRow class="hover:bg-transparent">
              <TableHead class="min-w-48 px-5 text-xs text-muted-foreground">Container</TableHead>
              <TableHead class="min-w-56 text-xs text-muted-foreground">Image</TableHead>
              <TableHead class="min-w-40 text-xs text-muted-foreground">Status</TableHead>
              <TableHead class="min-w-24 text-xs text-muted-foreground">State</TableHead>
              <TableHead class="min-w-48 text-xs text-muted-foreground">Ports</TableHead>
              <TableHead class="min-w-24 text-xs text-muted-foreground">CPU</TableHead>
              <TableHead class="min-w-36 text-xs text-muted-foreground">Memory</TableHead>
              <TableHead class="px-5 text-right text-xs text-muted-foreground">Restarts</TableHead>
              <TableHead class="w-14 px-5 text-right text-xs text-muted-foreground">
                <span class="sr-only">Actions</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-for="container in visibleContainers" :key="container.id">
              <TableCell class="px-5 py-3">
                <p class="max-w-56 truncate text-sm font-medium">{{ container.name }}</p>
                <div class="mt-1 flex items-center gap-1.5">
                  <p class="font-mono text-[10px] text-muted-foreground">
                    {{ container.id.slice(0, 12) }}
                  </p>
                  <button
                    class="grid size-5 shrink-0 place-items-center rounded-sm text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
                    type="button"
                    :aria-label="`Copy full ID for ${container.name}`"
                    :title="copiedContainerId === container.id ? 'Copied' : 'Copy container ID'"
                    @click="copyContainerId(container)"
                  >
                    <Check
                      v-if="copiedContainerId === container.id"
                      class="size-3"
                      :stroke-width="1.75"
                    />
                    <Copy v-else class="size-3" :stroke-width="1.5" />
                  </button>
                </div>
              </TableCell>
              <TableCell class="py-3">
                <p class="max-w-56 truncate font-mono text-[11px]" :title="container.image">
                  {{ container.image || "Unavailable" }}
                </p>
                <p v-if="container.managed" class="mt-1 text-[10px] text-muted-foreground">
                  Managed by Ignitify
                </p>
              </TableCell>
              <TableCell class="py-3">
                <span
                  class="flex items-center gap-2 text-xs"
                  :class="statusClass(container.status)"
                >
                  <span
                    class="status-dot"
                    :data-status="
                      container.status.startsWith('Up')
                        ? 'healthy'
                        : container.status.startsWith('Restarting')
                          ? 'live'
                          : undefined
                    "
                    aria-hidden="true"
                  />
                  <span class="max-w-36 truncate" :title="container.status">{{
                    container.status
                  }}</span>
                </span>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs text-muted-foreground">{{
                container.state
              }}</TableCell>
              <TableCell class="py-3">
                <p
                  v-if="container.ports.length"
                  class="max-w-44 truncate font-mono text-[11px]"
                  :title="container.ports.map(formatPort).join(', ')"
                >
                  {{ container.ports.map(formatPort).join(", ") }}
                </p>
                <span v-else class="text-xs text-muted-foreground">—</span>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs">
                {{
                  container.cpu_percentage === null
                    ? "—"
                    : `${container.cpu_percentage.toFixed(1)}%`
                }}
                <p class="mt-1 text-[10px] text-muted-foreground">
                  {{ formatCpuLimit(container.cpu_limit_nano_cpus) }}
                </p>
              </TableCell>
              <TableCell class="py-3 font-mono text-xs">
                {{ formatBytes(container.memory_usage_bytes) }}
                <p class="mt-1 text-[10px] text-muted-foreground">
                  {{ formatBytes(container.memory_limit_bytes) }} limit
                </p>
              </TableCell>
              <TableCell class="px-5 py-3 text-right font-mono text-xs">
                {{ container.restart_count }}
              </TableCell>
              <TableCell class="px-5 py-3 text-right">
                <Popover>
                  <PopoverTrigger as-child>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      :aria-label="`Actions for ${container.name}`"
                      title="Container actions"
                    >
                      <MoreHorizontal class="size-4" :stroke-width="1.5" />
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent align="end" class="w-56 p-1">
                    <p
                      class="px-2 py-1.5 text-[10px] font-medium uppercase tracking-[0.12em] text-muted-foreground"
                    >
                      Container actions
                    </p>
                    <PopoverClose v-for="action in containerActions" :key="action.key" as-child>
                      <button
                        class="flex w-full items-center gap-2 rounded-sm px-2 py-2 text-left text-sm transition-colors hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
                        type="button"
                        @click="openContainerAction(container, action.key)"
                      >
                        <component
                          :is="action.icon"
                          class="size-4 text-muted-foreground"
                          :stroke-width="1.5"
                        />
                        <span>{{ action.label }}</span>
                      </button>
                    </PopoverClose>
                    <div class="my-1 border-t border-border" />
                    <PopoverClose as-child>
                      <button
                        class="flex w-full items-center gap-2 rounded-sm px-2 py-2 text-left text-sm text-destructive transition-colors hover:bg-destructive/10 focus-visible:bg-destructive/10 focus-visible:outline-none"
                        type="button"
                        @click="openContainerAction(container, 'remove')"
                      >
                        <Trash2 class="size-4" :stroke-width="1.5" />
                        <span>Remove Container</span>
                      </button>
                    </PopoverClose>
                  </PopoverContent>
                </Popover>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>
        <nav
          v-if="containers && pageCount > 1"
          class="flex items-center justify-between gap-4 border-t border-border px-5 py-3 max-[640px]:items-start max-[640px]:flex-col"
          aria-label="Container inventory pagination"
        >
          <p class="text-xs text-muted-foreground" aria-live="polite">
            Showing {{ firstVisibleContainer }}–{{ lastVisibleContainer }} of
            {{ containerCount }} containers
          </p>
          <div class="flex items-center gap-2">
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="currentPage === 1"
              aria-label="Previous page"
              @click="goToPreviousPage"
            >
              <ChevronLeft class="size-4" :stroke-width="1.5" />
            </Button>
            <span class="min-w-20 text-center font-mono text-xs text-muted-foreground">
              Page {{ currentPage }} of {{ pageCount }}
            </span>
            <Button
              size="icon-sm"
              variant="outline"
              :disabled="currentPage === pageCount"
              aria-label="Next page"
              @click="goToNextPage"
            >
              <ChevronRight class="size-4" :stroke-width="1.5" />
            </Button>
          </div>
        </nav>
        <div v-if="containers !== null && containers.length === 0" class="px-5 py-8">
          <Container class="size-4 text-muted-foreground" :stroke-width="1.5" />
          <p class="mt-4 text-sm font-medium">No containers found</p>
          <p class="mt-1 text-xs leading-5 text-muted-foreground">
            Docker is reachable but has no active or stopped containers to monitor.
          </p>
        </div>
      </section>
    </section>

    <ContainerActionDialog
      v-model:open="actionDialogOpen"
      v-model:remove-open="removeDialogOpen"
      :action="activeAction"
      :container="selectedContainer"
      @removed="handleContainerRemoved"
    />
  </div>
</template>
