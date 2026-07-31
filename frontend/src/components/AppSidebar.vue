<script setup lang="ts">
import {
  Activity,
  Box,
  ChevronDown,
  CircleHelp,
  GitBranch,
  LayoutDashboard,
  LogOut,
  PanelLeftClose,
  Server,
  Settings2,
} from "@lucide/vue";
import { RouterLink, useRoute } from "vue-router";
import { useAuthStore } from "@/stores/auth";

interface Props {
  collapsed: boolean;
  open: boolean;
}

withDefaults(defineProps<Props>(), { collapsed: false });
const emit = defineEmits<{ close: []; toggleCollapse: [] }>();
const route = useRoute();
const auth = useAuthStore();

const primaryNavigation = [
  { label: "Overview", to: "/dashboard", icon: LayoutDashboard },
  { label: "Projects", to: "/projects", icon: Box },
];

const operations = [
  { label: "Deployments", icon: GitBranch },
  { label: "Servers", icon: Server },
  { label: "Activity", icon: Activity },
];
</script>

<template>
  <aside
    class="fixed inset-y-0 left-0 z-30 flex w-[244px] -translate-x-full flex-col border-r border-[var(--sidebar-border)] bg-[var(--sidebar-background)] px-3.5 pt-[22px] pb-4 text-[var(--sidebar-foreground)] transition-[width,padding,transform] duration-200 md:translate-x-0"
    :class="[
      collapsed ? 'md:w-[68px] md:px-2.5' : '',
      open ? 'translate-x-0 shadow-[20px_0_40px_rgb(0_0_0_/_18%)]' : '',
    ]"
    aria-label="Primary navigation"
  >
    <div
      class="flex items-center justify-between px-2"
      :class="collapsed ? 'md:justify-center md:px-0' : ''"
    >
      <button
        class="flex min-w-0 items-center gap-2.5 text-[var(--sidebar-strong)]"
        :class="collapsed ? 'md:justify-center' : ''"
        type="button"
        :aria-label="collapsed ? 'Expand sidebar' : 'Collapse sidebar'"
        :aria-pressed="collapsed"
        @click="emit('toggleCollapse')"
      >
        <img src="@/assets/logo/logo-white.svg" alt="" class="size-[21px] shrink-0" />
        <span
          class="font-mono text-xs tracking-[0.08em] uppercase"
          :class="collapsed ? 'md:hidden' : ''"
          >Ignitify</span
        >
      </button>
      <button
        class="grid size-[30px] place-items-center text-[var(--sidebar-muted)] hover:text-[var(--sidebar-strong)] md:hidden"
        type="button"
        aria-label="Close navigation"
        @click="emit('close')"
      >
        <PanelLeftClose :size="17" :stroke-width="1.5" />
      </button>
    </div>

    <button
      class="mt-[30px] flex w-full items-center gap-2.5 rounded-[5px] border border-[var(--sidebar-border)] bg-[var(--sidebar-surface)] p-2.5 text-left text-[var(--sidebar-strong)] hover:border-[var(--sidebar-hover-border)]"
      :class="collapsed ? 'md:justify-center md:p-2' : ''"
      type="button"
      aria-label="Current workspace"
      :title="collapsed ? 'Nova Flow workspace' : undefined"
    >
      <span
        class="grid size-[27px] shrink-0 place-items-center rounded-[4px] bg-[#d7a158] font-mono text-[10px] font-semibold text-[#30251c]"
        >NF</span
      >
      <span class="grid min-w-0 flex-1 gap-0.5" :class="collapsed ? 'md:hidden' : ''">
        <span class="font-mono text-[9px] tracking-[0.08em] text-[var(--sidebar-muted)] uppercase"
          >Workspace</span
        >
        <strong class="truncate text-xs font-medium">Nova Flow</strong>
      </span>
      <ChevronDown
        class="shrink-0"
        :class="collapsed ? 'md:hidden' : ''"
        :size="15"
        :stroke-width="1.5"
      />
    </button>

    <nav class="mt-[30px] grid gap-[3px]" aria-label="Workspace">
      <p
        class="mx-2.5 mb-2 font-mono text-[9px] tracking-[0.12em] text-[var(--sidebar-label)] uppercase"
        :class="collapsed ? 'md:hidden' : ''"
      >
        Workspace
      </p>
      <RouterLink
        v-for="item in primaryNavigation"
        :key="item.to"
        :to="item.to"
        class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-xs text-[var(--sidebar-muted)] hover:bg-[var(--sidebar-active)] hover:text-[var(--sidebar-strong)]"
        :class="[
          route.path === item.to
            ? 'bg-[var(--sidebar-active)] text-[var(--sidebar-strong)] shadow-[inset_2px_0_0_var(--sidebar-accent)]'
            : '',
          collapsed ? 'md:justify-center md:px-0' : '',
        ]"
        :title="collapsed ? item.label : undefined"
        @click="emit('close')"
      >
        <component :is="item.icon" class="shrink-0" :size="17" :stroke-width="1.6" />
        <span :class="collapsed ? 'md:hidden' : ''">{{ item.label }}</span>
      </RouterLink>
    </nav>

    <nav class="mt-[30px] grid gap-[3px]" aria-label="Operations">
      <p
        class="mx-2.5 mb-2 font-mono text-[9px] tracking-[0.12em] text-[var(--sidebar-label)] uppercase"
        :class="collapsed ? 'md:hidden' : ''"
      >
        Operations
      </p>
      <button
        v-for="item in operations"
        :key="item.label"
        class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-left text-xs text-[var(--sidebar-muted)] opacity-70 disabled:cursor-default"
        :class="collapsed ? 'md:justify-center md:px-0' : ''"
        type="button"
        :title="collapsed ? item.label : undefined"
        disabled
      >
        <component :is="item.icon" class="shrink-0" :size="17" :stroke-width="1.6" />
        <span :class="collapsed ? 'md:hidden' : ''">{{ item.label }}</span>
        <span
          class="ml-auto font-mono text-[9px] text-[var(--sidebar-label)] uppercase"
          :class="collapsed ? 'md:hidden' : ''"
          >Soon</span
        >
      </button>
    </nav>

    <div class="mt-auto grid gap-[3px]">
      <button
        class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-left text-xs text-[var(--sidebar-muted)] opacity-70 disabled:cursor-default"
        :class="collapsed ? 'md:justify-center md:px-0' : ''"
        type="button"
        :title="collapsed ? 'Settings' : undefined"
        disabled
      >
        <Settings2 class="shrink-0" :size="17" :stroke-width="1.6" /><span
          :class="collapsed ? 'md:hidden' : ''"
          >Settings</span
        >
      </button>
      <button
        class="flex min-h-[35px] w-full items-center gap-[11px] rounded-[4px] px-2.5 text-left text-xs text-[var(--sidebar-muted)] opacity-70 disabled:cursor-default"
        :class="collapsed ? 'md:justify-center md:px-0' : ''"
        type="button"
        :title="collapsed ? 'Support' : undefined"
        disabled
      >
        <CircleHelp class="shrink-0" :size="17" :stroke-width="1.6" /><span
          :class="collapsed ? 'md:hidden' : ''"
          >Support</span
        >
      </button>
      <div
        class="mx-2.5 my-3 h-px bg-[var(--sidebar-border)]"
        :class="collapsed ? 'md:mx-0.5' : ''"
      />
      <div
        class="flex items-center gap-2.5 px-1.5 py-1"
        :class="collapsed ? 'md:justify-center md:px-0' : ''"
      >
        <span
          class="grid size-7 shrink-0 place-items-center rounded-full bg-[var(--sidebar-avatar)] font-mono text-[10px] font-semibold text-[var(--sidebar-strong)]"
          >{{ auth.currentUser?.username?.slice(0, 2).toUpperCase() || "AP" }}</span
        >
        <span class="grid min-w-0 flex-1 gap-0.5" :class="collapsed ? 'md:hidden' : ''"
          ><strong class="truncate text-xs font-medium">{{
            auth.currentUser?.username || "Arif"
          }}</strong
          ><small
            class="truncate font-mono text-[9px] tracking-[0.08em] text-[var(--sidebar-muted)] uppercase"
            >{{ auth.currentUser?.role || "Administrator" }}</small
          ></span
        >
        <button
          class="grid size-[26px] shrink-0 place-items-center text-[var(--sidebar-muted)] hover:text-[var(--sidebar-strong)]"
          :class="collapsed ? 'md:hidden' : ''"
          type="button"
          aria-label="Sign out"
          title="Sign out"
          @click="auth.logout"
        >
          <LogOut :size="15" :stroke-width="1.5" />
        </button>
      </div>
    </div>
  </aside>
</template>
