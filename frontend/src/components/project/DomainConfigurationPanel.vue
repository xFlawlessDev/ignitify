<script setup lang="ts">
import { Globe2, Server } from "@lucide/vue";
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ServiceSummary } from "@/lib/types";

const props = defineProps<{
  services: ServiceSummary[];
  serverDomain: string;
  serviceId: string;
  domainError: string;
  showServiceSelector?: boolean;
}>();

const emit = defineEmits<{
  "update:serverDomain": [value: string];
  "update:serviceId": [value: string];
  create: [];
}>();

const routableServices = computed(() => props.services.filter((service) => service.internal_port));
const canSubmit = computed(() => Boolean(props.serviceId) && !props.domainError);
</script>

<template>
  <section class="app-surface" aria-labelledby="domain-configuration-heading">
    <header class="app-panel-header flex items-start gap-3 px-5 py-4">
      <span
        class="grid size-8 shrink-0 place-items-center rounded-[6px] border border-border bg-muted text-muted-foreground"
      >
        <Globe2 class="size-4" :stroke-width="1.5" />
      </span>
      <div>
        <p class="ui-label">Ingress</p>
        <h2 id="domain-configuration-heading" class="mt-1.5 text-base font-medium">
          Add custom domain
        </h2>
        <p class="mt-1.5 max-w-[60ch] text-xs leading-5 text-muted-foreground">
          Connect a public hostname to a service. DNS and TLS policy are managed in Infrastructure.
        </p>
      </div>
    </header>

    <form class="grid gap-5 px-5 py-5" @submit.prevent="emit('create')">
      <div class="grid gap-2">
        <div class="flex items-center justify-between gap-3">
          <Label for="project-domain" class="text-xs font-medium">Custom domain</Label>
          <span class="font-mono text-[10px] text-muted-foreground">FQDN</span>
        </div>
        <Input
          id="project-domain"
          class="min-w-0 rounded-[3px] font-mono text-sm"
          :model-value="props.serverDomain"
          placeholder="app.example.com"
          autocomplete="off"
          spellcheck="false"
          :aria-invalid="Boolean(props.domainError)"
          aria-describedby="project-domain-help project-domain-error"
          @update:model-value="emit('update:serverDomain', String($event))"
        />
        <p id="project-domain-help" class="text-[11px] leading-4 text-muted-foreground">
          Hostname only, without <code class="font-mono text-foreground">http://</code> or a path.
        </p>
        <p v-if="props.domainError" id="project-domain-error" class="text-[11px] text-destructive">
          {{ props.domainError }}
        </p>
      </div>

      <div
        v-if="props.showServiceSelector !== false"
        class="grid gap-2 border-t border-border pt-5"
      >
        <Label for="domain-service" class="flex items-center gap-2 text-xs font-medium">
          <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
          Service
        </Label>
        <Select
          :model-value="props.serviceId || undefined"
          :disabled="!routableServices.length"
          @update:model-value="(value) => emit('update:serviceId', String(value ?? ''))"
        >
          <SelectTrigger id="domain-service" class="w-full rounded-[3px]">
            <SelectValue placeholder="Select service that handles the port" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="service in routableServices" :key="service.id" :value="service.id">
              <span class="flex items-center gap-2">
                <span>{{ service.name }}</span>
                <span class="font-mono text-[10px] text-muted-foreground"
                  >:{{ service.internal_port }}</span
                >
              </span>
            </SelectItem>
          </SelectContent>
        </Select>
        <p class="text-[11px] leading-4 text-muted-foreground">
          Only services with an internal port can receive public traffic.
        </p>
        <p v-if="!routableServices.length" class="text-[11px] text-destructive">
          Configure an internal port on a service before adding a domain.
        </p>
      </div>

      <p v-else class="border-t border-border pt-5 text-[11px] leading-4 text-muted-foreground">
        This route targets the current service.
      </p>

      <div class="flex items-center justify-between gap-4 border-t border-border pt-4">
        <p class="text-[11px] leading-4 text-muted-foreground">
          The route starts pending and is reconciled by the control plane.
        </p>
        <Button class="shrink-0" size="sm" type="submit" :disabled="!canSubmit">
          Add domain
        </Button>
      </div>
    </form>
  </section>
</template>
