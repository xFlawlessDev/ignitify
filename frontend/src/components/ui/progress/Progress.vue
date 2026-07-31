<script setup lang="ts">
import type { ProgressRootProps } from "reka-ui";
import type { HTMLAttributes } from "vue";
import { computed } from "vue";
import { reactiveOmit } from "@vueuse/core";
import { ProgressIndicator, ProgressRoot } from "reka-ui";
import { cn } from "@/lib/utils";

const props = withDefaults(defineProps<ProgressRootProps & { class?: HTMLAttributes["class"] }>(), {
  modelValue: 0,
});

const delegatedProps = reactiveOmit(props, "class");

// Null / undefined model-value = indeterminate state.
const isIndeterminate = computed(() => props.modelValue == null);
</script>

<template>
  <ProgressRoot
    data-slot="progress"
    v-bind="delegatedProps"
    :class="cn('bg-primary/20 relative h-2 w-full overflow-hidden rounded-full', props.class)"
  >
    <ProgressIndicator
      data-slot="progress-indicator"
      :class="
        cn(
          'bg-primary h-full w-full flex-1',
          isIndeterminate ? 'origin-left animate-pulse' : 'transition-all',
        )
      "
      :style="
        isIndeterminate ? undefined : `transform: translateX(-${100 - (props.modelValue ?? 0)}%);`
      "
    />
  </ProgressRoot>
</template>
