<script setup lang="ts">
import { ChevronRight, Menu, Moon, Sun } from "@lucide/vue";
import { computed, onMounted, onUnmounted, shallowRef } from "vue";
import { RouterLink, useRoute } from "vue-router";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useControlPlanePreferences } from "@/composables/useControlPlanePreferences";

const emit = defineEmits<{ openNavigation: [] }>();
const route = useRoute();
const { isDark, toggleTheme } = useControlPlanePreferences();
const now = shallowRef(new Date());
let clockId: number | undefined;

const breadcrumbs = computed(() => {
  if (route.name === "ProjectDetail") {
    const projectId = String(route.params.projectId);
    const projectName = projectId
      .split("-")
      .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
      .join(" ");

    return [{ label: "Projects", to: "/projects" }, { label: projectName }];
  }

  return [{ label: route.name === "Projects" ? "Projects" : "Overview" }];
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

    <nav class="flex min-w-0 flex-1 items-center gap-1.5" aria-label="Breadcrumb">
      <template v-for="(breadcrumb, index) in breadcrumbs" :key="breadcrumb.label">
        <ChevronRight
          v-if="index"
          class="size-3.5 shrink-0 text-muted-foreground"
          :stroke-width="1.5"
          aria-hidden="true"
        />
        <RouterLink
          v-if="breadcrumb.to"
          :to="breadcrumb.to"
          class="truncate text-sm text-muted-foreground hover:text-foreground"
        >
          {{ breadcrumb.label }}
        </RouterLink>
        <span v-else class="truncate text-sm font-medium text-foreground">{{
          breadcrumb.label
        }}</span>
      </template>
    </nav>

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
