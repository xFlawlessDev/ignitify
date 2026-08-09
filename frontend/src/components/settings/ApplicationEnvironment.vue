<script setup lang="ts">
import { Globe2, LockKeyhole, Settings2 } from "@lucide/vue";
import type { ApplicationEnvironmentStatus } from "@/lib/api/settings";

const props = defineProps<{
  environment: ApplicationEnvironmentStatus | null;
}>();
</script>

<template>
  <section class="app-surface" aria-labelledby="application-environment-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <Settings2 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Application environment</p>
        <h2 id="application-environment-heading" class="mt-1.5 text-base font-medium">
          Runtime defaults
        </h2>
      </div>
    </header>

    <dl class="grid divide-y divide-border sm:grid-cols-2 sm:divide-x sm:divide-y-0">
      <div class="flex items-start gap-3 px-5 py-4">
        <Globe2 class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div class="min-w-0">
          <dt class="text-xs font-medium">Public origin</dt>
          <dd class="mt-1 break-all font-mono text-[11px] text-foreground">
            {{
              props.environment ? props.environment.public_origin || "Not configured" : "checking"
            }}
          </dd>
        </div>
      </div>

      <div class="flex items-start gap-3 px-5 py-4">
        <LockKeyhole class="mt-0.5 size-4 shrink-0 text-muted-foreground" :stroke-width="1.5" />
        <div class="min-w-0">
          <dt class="text-xs font-medium">Secure cookies</dt>
          <dd
            class="mt-1 font-mono text-[11px]"
            :class="
              props.environment?.secure_cookies ? 'text-metric-green' : 'text-muted-foreground'
            "
          >
            {{
              props.environment
                ? props.environment.secure_cookies
                  ? "enabled"
                  : "disabled"
                : "checking"
            }}
          </dd>
        </div>
      </div>
    </dl>
  </section>
</template>
