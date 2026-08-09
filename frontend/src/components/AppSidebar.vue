<script setup lang="ts">
import {
  Box,
  Cpu,
  Container,
  Activity,
  GitBranch,
  HeartPulse,
  LayoutDashboard,
  LogOut,
  PanelLeftClose,
  RefreshCw,
  Settings2,
  Server,
  TerminalSquare,
} from "@lucide/vue";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { RouterLink, useRoute } from "vue-router";
import { toast } from "vue-sonner";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useAppUpdate } from "@/composables/useAppUpdate";
import { useAuthStore } from "@/stores/auth";

interface Props {
  collapsed: boolean;
  open: boolean;
}

withDefaults(defineProps<Props>(), { collapsed: false });
const emit = defineEmits<{ close: []; toggleCollapse: [] }>();
const route = useRoute();
const auth = useAuthStore();
const { t } = useI18n();
const { appVersion, checkForUpdate, isChecking } = useAppUpdate();

const primaryNavigation = computed(() => {
  const items = [
    { labelKey: "navigation.overview", to: "/dashboard", icon: LayoutDashboard },
    { labelKey: "navigation.monitoring", to: "/monitoring", icon: Activity },
    { labelKey: "navigation.uptime", to: "/uptime", icon: HeartPulse },
    { labelKey: "navigation.projects", to: "/projects", icon: Box },
  ];
  if (auth.isAdmin) {
    items.push({ labelKey: "navigation.providers", to: "/providers", icon: GitBranch });
    items.push({ labelKey: "navigation.docker", to: "/containers", icon: Container });
    items.push({ labelKey: "navigation.terminal", to: "/terminal", icon: TerminalSquare });
    items.push({ labelKey: "navigation.remoteBuilders", to: "/remote-builders", icon: Cpu });
    items.push({ labelKey: "navigation.remoteServers", to: "/remote-servers", icon: Server });
  }
  return items;
});

async function checkForUpdates() {
  const result = await checkForUpdate();

  if (result.kind === "updateAvailable") {
    toast.info(t("appUpdate.updateAvailable"), {
      description: t("appUpdate.updateAvailableDescription", { version: result.version }),
      action: {
        label: t("appUpdate.viewRelease"),
        onClick: () => window.open(result.releaseUrl, "_blank", "noopener,noreferrer"),
      },
    });
    return;
  }

  if (result.kind === "upToDate") {
    toast.success(t("appUpdate.upToDate"), {
      description: t("appUpdate.upToDateDescription", { version: result.version }),
    });
    return;
  }

  if (result.kind === "noRelease") {
    toast.info(t("appUpdate.noRelease"), { description: t("appUpdate.noReleaseDescription") });
    return;
  }

  toast.error(t("appUpdate.checkFailed"), { description: t("appUpdate.checkFailedDescription") });
}

function isNavigationItemActive(path: string) {
  return route.path === path || route.path.startsWith(`${path}/`);
}
</script>

<template>
  <aside
    class="fixed inset-y-0 left-0 z-30 flex w-[244px] max-w-[calc(100vw-1rem)] -translate-x-full flex-col overflow-x-hidden overflow-y-auto border-r border-[var(--sidebar-border)] bg-[var(--sidebar-background)] px-3.5 pt-[22px] pb-4 text-[var(--sidebar-foreground)] transition-[width,padding,transform] duration-200 ease-out motion-reduce:transition-none md:translate-x-0"
    :class="[collapsed ? 'md:w-[68px] md:px-2.5' : '', open ? 'translate-x-0' : '']"
    :aria-label="t('navigation.workspace')"
  >
    <div class="flex items-center justify-between px-2" :class="collapsed ? 'md:px-0' : ''">
      <Tooltip :disabled="!collapsed">
        <TooltipTrigger as-child>
          <Button
            variant="ghost"
            class="flex h-10 min-w-0 flex-1 items-center gap-2.5 px-0 py-0 text-[var(--sidebar-strong)] transition-[gap] duration-200 ease-out hover:!bg-transparent hover:!text-[var(--sidebar-strong)] motion-reduce:transition-none"
            :class="collapsed ? 'md:justify-center md:gap-0' : ''"
            type="button"
            :aria-label="
              collapsed ? t('navigation.expandSidebar') : t('navigation.collapseSidebar')
            "
            :aria-pressed="collapsed"
            @click="emit('toggleCollapse')"
          >
            <img src="@/assets/logo/logo-white.svg" alt="" class="size-9 shrink-0" />
            <span
              class="block max-w-[12rem] overflow-hidden font-mono text-xs tracking-[0.08em] whitespace-nowrap uppercase opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
              :class="
                collapsed ? 'md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0' : ''
              "
              >Ignitify</span
            >
            <PanelLeftClose
              class="ml-auto max-w-4 shrink-0 overflow-hidden text-[var(--sidebar-muted)] opacity-100 transition-[max-width,margin-left,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
              :class="
                collapsed
                  ? 'md:ml-0 md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0'
                  : ''
              "
              :size="16"
              :stroke-width="1.5"
            />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="right" :side-offset="10">
          {{ t("navigation.expandSidebar") }}
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger as-child>
          <Button
            variant="ghost"
            class="grid size-[30px] shrink-0 place-items-center text-[var(--sidebar-muted)] hover:text-[var(--sidebar-strong)] md:hidden"
            type="button"
            :aria-label="t('navigation.closeNavigation')"
            @click="emit('close')"
          >
            <PanelLeftClose :size="17" :stroke-width="1.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="right">{{ t("navigation.closeNavigation") }}</TooltipContent>
      </Tooltip>
    </div>

    <nav class="mt-7 grid gap-[3px]" :aria-label="t('navigation.workspace')">
      <p
        class="mx-2.5 mb-2 max-h-4 max-w-[12rem] overflow-hidden font-mono text-[9px] tracking-[0.12em] whitespace-nowrap text-[var(--sidebar-label)] uppercase opacity-100 transition-[max-width,max-height,margin-bottom,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
        :class="collapsed ? 'md:mb-0 md:max-h-0 md:max-w-0 md:translate-x-1 md:opacity-0' : ''"
      >
        {{ t("navigation.workspace") }}
      </p>
      <Tooltip v-for="item in primaryNavigation" :key="item.to" :disabled="!collapsed">
        <TooltipTrigger as-child>
          <RouterLink
            :to="item.to"
            class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-xs text-[var(--sidebar-muted)] transition-[background-color,color,gap,padding] duration-150 ease-out motion-reduce:transition-none hover:bg-[var(--sidebar-active)] hover:text-[var(--sidebar-strong)]"
            :class="[
              isNavigationItemActive(item.to)
                ? 'bg-[var(--sidebar-active)] text-[var(--sidebar-strong)] shadow-[inset_2px_0_0_var(--sidebar-accent)]'
                : '',
              collapsed ? 'md:justify-center md:gap-0 md:px-0' : '',
            ]"
            @click="emit('close')"
          >
            <component :is="item.icon" class="shrink-0" :size="17" :stroke-width="1.6" />
            <span
              class="block max-w-[12rem] overflow-hidden whitespace-nowrap opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
              :class="
                collapsed ? 'md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0' : ''
              "
              >{{ t(item.labelKey) }}</span
            >
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right" :side-offset="10">{{ t(item.labelKey) }}</TooltipContent>
      </Tooltip>
    </nav>

    <nav v-if="auth.isAdmin" class="mt-5 grid gap-[3px]" :aria-label="t('navigation.system')">
      <div
        class="mx-2.5 mb-3 h-px bg-[var(--sidebar-border)]"
        :class="collapsed ? 'md:mx-0.5' : ''"
      />
      <p
        class="mx-2.5 mb-2 max-h-4 max-w-[12rem] overflow-hidden font-mono text-[9px] tracking-[0.12em] whitespace-nowrap text-[var(--sidebar-label)] uppercase opacity-100 transition-[max-width,max-height,margin-bottom,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
        :class="collapsed ? 'md:mb-0 md:max-h-0 md:max-w-0 md:translate-x-1 md:opacity-0' : ''"
      >
        {{ t("navigation.system") }}
      </p>
      <Tooltip :disabled="!collapsed">
        <TooltipTrigger as-child>
          <RouterLink
            to="/settings"
            class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-xs text-[var(--sidebar-muted)] transition-[background-color,color,gap,padding] duration-150 ease-out motion-reduce:transition-none hover:bg-[var(--sidebar-active)] hover:text-[var(--sidebar-strong)]"
            :class="[
              isNavigationItemActive('/settings')
                ? 'bg-[var(--sidebar-active)] text-[var(--sidebar-strong)] shadow-[inset_2px_0_0_var(--sidebar-accent)]'
                : '',
              collapsed ? 'md:justify-center md:gap-0 md:px-0' : '',
            ]"
            @click="emit('close')"
          >
            <Settings2 class="shrink-0" :size="17" :stroke-width="1.6" />
            <span
              class="block max-w-[12rem] overflow-hidden whitespace-nowrap opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
              :class="
                collapsed ? 'md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0' : ''
              "
              >{{ t("navigation.settings") }}</span
            >
          </RouterLink>
        </TooltipTrigger>
        <TooltipContent side="right" :side-offset="10">
          {{ t("navigation.settings") }}
        </TooltipContent>
      </Tooltip>
    </nav>

    <div class="mt-auto grid gap-[3px]">
      <div
        class="mt-2 flex min-h-[35px] items-center gap-2 px-2.5 text-[var(--sidebar-muted)] transition-[gap,padding] duration-200 ease-out motion-reduce:transition-none"
        :class="collapsed ? 'md:justify-center md:gap-0 md:px-0' : ''"
      >
        <span
          class="min-w-0 max-w-[12rem] flex-1 truncate font-mono text-[10px] tracking-[0.04em] whitespace-nowrap opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
          :class="
            collapsed ? 'md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0' : ''
          "
        >
          {{ t("appUpdate.version", { version: `v${appVersion}` }) }}
        </span>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              variant="ghost"
              class="grid size-[26px] shrink-0 place-items-center rounded-[3px] hover:bg-[var(--sidebar-active)] hover:text-[var(--sidebar-strong)] disabled:cursor-wait disabled:opacity-70"
              type="button"
              :aria-label="
                isChecking ? t('appUpdate.checkingForUpdates') : t('appUpdate.checkForUpdates')
              "
              :disabled="isChecking"
              @click="checkForUpdates"
            >
              <RefreshCw :size="14" :stroke-width="1.5" :class="isChecking ? 'animate-spin' : ''" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right" :side-offset="10">
            {{ isChecking ? t("appUpdate.checkingForUpdates") : t("appUpdate.checkForUpdates") }}
          </TooltipContent>
        </Tooltip>
      </div>
      <div
        class="mx-2.5 my-3 h-px bg-[var(--sidebar-border)]"
        :class="collapsed ? 'md:mx-0.5' : ''"
      />
      <div
        class="flex items-center gap-2.5 px-1.5 py-1 transition-[gap,padding] duration-200 ease-out motion-reduce:transition-none"
        :class="collapsed ? 'md:justify-center md:gap-0 md:px-0' : ''"
      >
        <Popover v-if="collapsed">
          <PopoverTrigger as-child>
            <Button
              variant="ghost"
              class="size-7 shrink-0 rounded-full bg-[var(--sidebar-avatar)] p-0 font-mono text-[10px] font-semibold text-[var(--sidebar-strong)] hover:bg-[var(--sidebar-avatar)] hover:text-[var(--sidebar-strong)]"
              type="button"
              :aria-label="auth.currentUser?.username || 'Account'"
            >
              {{ auth.currentUser?.username?.slice(0, 2).toUpperCase() || "AP" }}
            </Button>
          </PopoverTrigger>
          <PopoverContent side="right" align="end" :side-offset="10" class="w-56 p-1">
            <div class="flex min-w-0 items-center gap-3 px-3 py-2.5">
              <span
                class="grid size-7 shrink-0 place-items-center rounded-full bg-muted font-mono text-[10px] font-semibold text-foreground"
                >{{ auth.currentUser?.username?.slice(0, 2).toUpperCase() || "AP" }}</span
              >
              <div class="min-w-0">
                <p class="truncate text-sm font-medium text-foreground">
                  {{ auth.currentUser?.username || "Arif" }}
                </p>
                <p class="truncate font-mono text-[10px] uppercase text-muted-foreground">
                  {{ auth.currentUser?.role || "Administrator" }}
                </p>
              </div>
            </div>
            <div class="my-1 border-t border-border" />
            <Button
              variant="ghost"
              class="flex w-full items-center justify-start gap-2 rounded-sm px-3 py-2 text-sm text-destructive hover:bg-destructive/10 hover:text-destructive focus-visible:bg-destructive/10"
              type="button"
              @click="auth.logout"
            >
              <LogOut class="size-4" :stroke-width="1.5" />
              {{ t("navigation.signOut") }}
            </Button>
          </PopoverContent>
        </Popover>
        <span
          v-else
          class="grid size-7 shrink-0 place-items-center rounded-full bg-[var(--sidebar-avatar)] font-mono text-[10px] font-semibold text-[var(--sidebar-strong)]"
          >{{ auth.currentUser?.username?.slice(0, 2).toUpperCase() || "AP" }}</span
        >
        <span
          class="grid min-w-0 max-w-[12rem] flex-1 gap-0.5 overflow-hidden whitespace-nowrap opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
          :class="
            collapsed ? 'md:max-w-0 md:pointer-events-none md:translate-x-1 md:opacity-0' : ''
          "
          ><strong class="truncate text-xs font-medium">{{
            auth.currentUser?.username || "Arif"
          }}</strong
          ><small
            class="truncate font-mono text-[9px] tracking-[0.08em] text-[var(--sidebar-muted)] uppercase"
            >{{ auth.currentUser?.role || "Administrator" }}</small
          ></span
        >
        <span
          v-if="!collapsed"
          class="block max-w-[12rem] overflow-hidden whitespace-nowrap opacity-100 transition-[max-width,opacity,transform] duration-200 ease-out motion-reduce:transition-none"
        >
          <Tooltip>
            <TooltipTrigger as-child>
              <Button
                variant="ghost"
                class="grid size-[26px] shrink-0 place-items-center text-[var(--sidebar-muted)] hover:text-[var(--sidebar-strong)]"
                type="button"
                :aria-label="t('navigation.signOut')"
                @click="auth.logout"
              >
                <LogOut :size="15" :stroke-width="1.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="right" :side-offset="10">
              {{ t("navigation.signOut") }}
            </TooltipContent>
          </Tooltip>
        </span>
      </div>
    </div>
  </aside>
</template>
