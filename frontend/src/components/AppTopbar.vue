<script setup lang="ts">
import { Menu, Moon, Sun } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { RouterLink, useRoute } from "vue-router";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbPage,
  BreadcrumbList,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useControlPlanePreferences } from "@/composables/useControlPlanePreferences";

const emit = defineEmits<{ openNavigation: [] }>();
const route = useRoute();
const { isDark, toggleTheme } = useControlPlanePreferences();
const now = shallowRef(new Date());
let clockId: number | undefined;

interface BreadcrumbEntry {
  key: string;
  label: string;
  to?: RouteLocationRaw;
}

function formatRouteSegment(value: string) {
  return value.replace(/[-_]+/g, " ").replace(/\b\w/g, (character) => character.toUpperCase());
}

function fallbackRouteLabel(value: string) {
  return formatRouteSegment(value.replace(/^\//, "").split("/").at(-1) ?? "Overview");
}

const breadcrumbs = computed<BreadcrumbEntry[]>(() => {
  const matchedRoutes = route.matched.filter((record) => record.meta.layout !== "blank");
  const entries: BreadcrumbEntry[] = matchedRoutes.map((record, index) => {
    const recordName = typeof record.name === "string" ? record.name : record.path;
    const rawLabel = record.meta.breadcrumb ?? fallbackRouteLabel(recordName);
    const routeParam = record.meta.breadcrumbParam
      ? route.params[record.meta.breadcrumbParam]
      : undefined;
    const paramValue = Array.isArray(routeParam) ? routeParam.at(-1) : routeParam;
    const label = paramValue ? formatRouteSegment(paramValue) : rawLabel;

    return {
      key: `${recordName}-${index}`,
      label,
      to:
        index < matchedRoutes.length - 1 && typeof record.name === "string"
          ? { name: record.name }
          : undefined,
    };
  });

  const parent = matchedRoutes.at(-1)?.meta.breadcrumbParent;
  if (parent && !entries.some((entry) => entry.label === parent.label)) {
    entries.unshift({ key: `parent-${parent.label}`, label: parent.label, to: parent.to });
  }

  return entries.length > 0 ? entries : [{ key: "overview", label: "Overview" }];
});

const serverTime = computed(() =>
  new Intl.DateTimeFormat("en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(now.value),
);

onMounted(() => {
  clockId = window.setInterval(() => {
    now.value = new Date();
  }, 1000);
});

onUnmounted(() => {
  if (clockId !== undefined) window.clearInterval(clockId);
});
</script>

<template>
  <header
    class="flex h-14 min-w-0 items-center gap-3 border-b border-border px-4 md:h-[62px] md:px-8"
  >
    <button
      class="grid size-8 shrink-0 place-items-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground md:hidden"
      type="button"
      aria-label="Open navigation"
      @click="emit('openNavigation')"
    >
      <Menu :size="18" :stroke-width="1.5" />
    </button>

    <Breadcrumb class="min-w-0 flex-1">
      <BreadcrumbList class="flex-nowrap gap-1.5">
        <template v-for="(breadcrumb, index) in breadcrumbs" :key="breadcrumb.key">
          <BreadcrumbSeparator v-if="index" />
          <BreadcrumbItem class="min-w-0">
            <BreadcrumbLink
              v-if="breadcrumb.to"
              as-child
              class="truncate text-sm text-muted-foreground"
            >
              <RouterLink :to="breadcrumb.to">{{ breadcrumb.label }}</RouterLink>
            </BreadcrumbLink>
            <BreadcrumbPage v-else class="truncate text-sm font-medium">
              {{ breadcrumb.label }}
            </BreadcrumbPage>
          </BreadcrumbItem>
        </template>
      </BreadcrumbList>
    </Breadcrumb>

    <time
      class="hidden shrink-0 items-center gap-2 font-mono text-[11px] text-muted-foreground sm:flex"
      :datetime="now.toISOString()"
      title="Server time in UTC"
    >
      <span class="text-[9px] uppercase text-muted-foreground">Server</span>
      {{ serverTime }} UTC
    </time>

    <Tooltip>
      <TooltipTrigger as-child>
        <button
          class="grid size-8 shrink-0 place-items-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="isDark ? 'Use light theme' : 'Use dark theme'"
          :aria-pressed="isDark"
          @click="toggleTheme"
        >
          <Sun v-if="isDark" :size="17" :stroke-width="1.5" />
          <Moon v-else :size="17" :stroke-width="1.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        {{ isDark ? "Use light theme" : "Use dark theme" }}
      </TooltipContent>
    </Tooltip>
  </header>
</template>
