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
    <div class="min-h-dvh overflow-x-clip bg-background">
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
        class="min-h-dvh min-w-0 transition-[margin-left] duration-200"
        :class="isSidebarCollapsed ? 'md:ml-[68px]' : 'md:ml-[244px]'"
      >
        <AppTopbar
          class="sticky top-0 z-10 bg-background"
          @open-navigation="isSidebarOpen = true"
        />
        <main class="mx-auto w-full max-w-[1200px] min-w-0 p-4 sm:p-5 md:p-8">
          <slot />
        </main>
      </div>
    </div>
  </TooltipProvider>
</template>
