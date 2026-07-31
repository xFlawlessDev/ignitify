<script setup lang="ts">
import { shallowRef } from "vue";
import AppSidebar from "@/components/AppSidebar.vue";
import AppTopbar from "@/components/AppTopbar.vue";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useControlPlanePreferences } from "@/composables/useControlPlanePreferences";

const isSidebarOpen = shallowRef(false);
const { isSidebarCollapsed, toggleSidebar } = useControlPlanePreferences();
</script>

<template>
  <TooltipProvider>
    <div class="min-h-dvh bg-background">
      <AppSidebar
        :collapsed="isSidebarCollapsed"
        :open="isSidebarOpen"
        @close="isSidebarOpen = false"
        @toggle-collapse="toggleSidebar"
      />
      <button
        v-if="isSidebarOpen"
        class="fixed inset-0 z-20 size-full bg-black/30 md:hidden"
        type="button"
        aria-label="Close navigation"
        @click="isSidebarOpen = false"
      />

      <div
        class="min-h-dvh transition-[margin] duration-200 md:ml-[244px]"
        :class="isSidebarCollapsed ? 'md:ml-[68px]' : ''"
      >
        <AppTopbar @open-navigation="isSidebarOpen = true" />
        <main class="w-full max-w-[1480px] p-4 sm:p-5 md:p-8">
          <slot />
        </main>
      </div>
    </div>
  </TooltipProvider>
</template>
