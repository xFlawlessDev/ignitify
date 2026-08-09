<script setup lang="ts">
import { FileWarning } from "@lucide/vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

interface Props {
  heading: string;
  message: string;
  headingError?: string;
  messageError?: string;
}

const props = defineProps<Props>();

const emit = defineEmits<{
  (event: "update:heading", value: string): void;
  (event: "update:message", value: string): void;
}>();
</script>

<template>
  <section class="app-surface" aria-labelledby="ingress-fallback-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <FileWarning class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Ingress fallback</p>
        <h2 id="ingress-fallback-heading" class="mt-1.5 text-base font-medium">
          Unmatched hostname page
        </h2>
      </div>
    </header>

    <div class="grid gap-5 px-5 py-5">
      <div class="grid gap-2">
        <Label for="fallback-page-heading" class="text-xs font-medium">Page heading</Label>
        <Input
          id="fallback-page-heading"
          :model-value="props.heading"
          class="rounded-[3px]"
          maxlength="100"
          autocomplete="off"
          :aria-invalid="Boolean(props.headingError)"
          aria-describedby="fallback-page-heading-help fallback-page-heading-error"
          @update:model-value="emit('update:heading', String($event))"
        />
        <p id="fallback-page-heading-help" class="text-[11px] leading-4 text-muted-foreground">
          Displayed when a hostname reaches the proxy without an active application route.
        </p>
        <p
          v-if="props.headingError"
          id="fallback-page-heading-error"
          class="text-[11px] text-destructive"
        >
          {{ props.headingError }}
        </p>
      </div>

      <div class="grid gap-2 border-t border-border pt-5">
        <Label for="fallback-page-message" class="text-xs font-medium">Page message</Label>
        <Textarea
          id="fallback-page-message"
          :model-value="props.message"
          class="min-h-24 resize-y rounded-[3px]"
          rows="3"
          maxlength="280"
          :aria-invalid="Boolean(props.messageError)"
          aria-describedby="fallback-page-message-help fallback-page-message-error"
          @update:model-value="emit('update:message', String($event))"
        />
        <p id="fallback-page-message-help" class="text-[11px] leading-4 text-muted-foreground">
          Plain text only. Line breaks are preserved; HTML and scripts are never rendered.
        </p>
        <p
          v-if="props.messageError"
          id="fallback-page-message-error"
          class="text-[11px] text-destructive"
        >
          {{ props.messageError }}
        </p>
      </div>
    </div>
  </section>
</template>
