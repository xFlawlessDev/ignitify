<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { ConfigProvider } from "reka-ui";
import { Toaster } from "@/components/ui/sonner";
import MainLayout from "@/layouts/MainLayout.vue";
import BlankLayout from "@/layouts/BlankLayout.vue";

const route = useRoute();
const cspNonce = document.querySelector<HTMLMetaElement>("meta[name=ignitify-csp-nonce]")?.content;
const rekaCspNonce = cspNonce === "__IGNITIFY_CSP_NONCE__" ? undefined : cspNonce;

const layout = computed(() => {
  return route.meta.layout === "blank" ? BlankLayout : MainLayout;
});
</script>

<template>
  <ConfigProvider :nonce="rekaCspNonce">
    <component :is="layout">
      <RouterView v-slot="{ Component }">
        <Transition
          mode="out-in"
          enter-active-class="transition-opacity duration-150"
          leave-active-class="transition-opacity duration-150"
          enter-from-class="opacity-0"
          leave-to-class="opacity-0"
        >
          <component :is="Component" />
        </Transition>
      </RouterView>
    </component>
    <Toaster position="bottom-right" :rich-colors="true" />
  </ConfigProvider>
</template>
