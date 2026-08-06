<script setup lang="ts">
import { Languages, Menu, Moon, Sun } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
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
import { useLocale } from "@/composables/useLocale";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const emit = defineEmits<{ openNavigation: [] }>();
const route = useRoute();
const { t } = useI18n();
const { isDark, toggleTheme } = useControlPlanePreferences();
const { currentLocale, localeOptions, changeLocale } = useLocale();
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
    const label = paramValue ? formatRouteSegment(paramValue) : t(rawLabel);

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
  const parentLabel = parent ? t(parent.label) : undefined;
  if (parent && parentLabel && !entries.some((entry) => entry.label === parentLabel)) {
    entries.unshift({ key: `parent-${parent.label}`, label: parentLabel, to: parent.to });
  }

  return entries.length > 0 ? entries : [{ key: "overview", label: t("navigation.overview") }];
});

const serverTime = computed(() =>
  new Intl.DateTimeFormat(currentLocale.value === "id" ? "id-ID" : "en-GB", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
    timeZone: "UTC",
  }).format(now.value),
);

const selectedLocale = computed({
  get: () => currentLocale.value,
  set: (value: string | undefined) => changeLocale(value),
});

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
      :aria-label="t('topbar.openNavigation')"
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
      :title="t('topbar.serverTime')"
    >
      <span class="text-[9px] uppercase text-muted-foreground">{{ t("topbar.server") }}</span>
      {{ serverTime }} UTC
    </time>

    <Select v-model="selectedLocale">
      <SelectTrigger
        class="h-8 w-[132px] rounded-sm border-0 bg-transparent px-2 text-xs shadow-none hover:bg-muted"
        :aria-label="t('topbar.language')"
      >
        <Languages :size="15" :stroke-width="1.5" class="text-muted-foreground" />
        <SelectValue :placeholder="t('topbar.language')" />
      </SelectTrigger>
      <SelectContent align="end">
        <SelectItem v-for="option in localeOptions" :key="option.value" :value="option.value">
          {{ t(option.labelKey) }}
        </SelectItem>
      </SelectContent>
    </Select>

    <Tooltip>
      <TooltipTrigger as-child>
        <button
          class="grid size-8 shrink-0 place-items-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
          type="button"
          :aria-label="isDark ? t('topbar.useLightTheme') : t('topbar.useDarkTheme')"
          :aria-pressed="isDark"
          @click="toggleTheme"
        >
          <Sun v-if="isDark" :size="17" :stroke-width="1.5" />
          <Moon v-else :size="17" :stroke-width="1.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">
        {{ isDark ? t("topbar.useLightTheme") : t("topbar.useDarkTheme") }}
      </TooltipContent>
    </Tooltip>
  </header>
</template>
