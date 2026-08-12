<script setup lang="ts">
import { Eye, EyeOff, LockKeyhole, Plus, Trash2 } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { ServiceVariable } from "@/lib/types";

interface ServiceVariableDraft extends ServiceVariable {
  is_set?: boolean;
}

const props = defineProps<{
  activeEnvironmentKind: "variables" | "secrets";
  activeServiceVariables: Array<{ variable: ServiceVariableDraft; index: number }>;
  serviceSecretCount: number;
  serviceVariableCount: number;
  showSecretValues: boolean;
}>();

const emit = defineEmits<{
  addVariable: [isSecret: boolean];
  removeVariable: [index: number];
  updateActiveEnvironmentKind: [value: "variables" | "secrets"];
  updateSecret: [index: number, isSecret: boolean];
  updateShowSecretValues: [value: boolean];
}>();
</script>

<template>
  <fieldset class="grid gap-3 border-t border-border pt-4">
    <legend class="sr-only">Service environment</legend>
    <div class="flex items-start justify-between gap-4 max-[560px]:flex-col">
      <div>
        <p class="text-sm font-medium">Service environment</p>
        <p class="mt-1 text-xs leading-5 text-muted-foreground">
          Service keys override project defaults during deployment.
        </p>
      </div>
      <div class="grid shrink-0 gap-2 max-[560px]:w-full">
        <Tabs
          :model-value="props.activeEnvironmentKind"
          class="max-[560px]:w-full"
          @update:model-value="
            emit('updateActiveEnvironmentKind', $event as 'variables' | 'secrets')
          "
        >
          <TabsList class="h-8 w-full rounded-[4px] sm:w-auto">
            <TabsTrigger value="variables" class="min-w-28 px-3 text-[11px]">
              Variables
              <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                props.serviceVariableCount
              }}</span>
            </TabsTrigger>
            <TabsTrigger value="secrets" class="min-w-28 px-3 text-[11px]">
              <LockKeyhole class="size-3.5" :stroke-width="1.5" />
              Secrets
              <span class="ml-1 font-mono text-[10px] text-muted-foreground">{{
                props.serviceSecretCount
              }}</span>
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <Button
          v-if="props.activeEnvironmentKind === 'secrets' && props.serviceSecretCount"
          variant="ghost"
          class="inline-flex items-center justify-end gap-1.5 py-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
          type="button"
          @click="emit('updateShowSecretValues', !props.showSecretValues)"
        >
          <EyeOff v-if="props.showSecretValues" class="size-3.5" :stroke-width="1.5" />
          <Eye v-else class="size-3.5" :stroke-width="1.5" />
          {{ props.showSecretValues ? "Hide values" : "Reveal values" }}
        </Button>
      </div>
    </div>

    <div v-if="props.activeServiceVariables.length" class="grid gap-2">
      <div
        class="hidden grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] gap-2 py-1 text-[10px] uppercase text-muted-foreground sm:grid"
      >
        <span>Key</span><span>Value</span><span>Type</span><span class="sr-only">Actions</span>
      </div>
      <div
        v-for="{ variable, index } in props.activeServiceVariables"
        :key="index"
        class="grid min-h-[58px] grid-cols-[minmax(0,0.8fr)_minmax(0,1fr)_auto_auto] items-end gap-2 border-b border-border py-2.5 last:border-b-0 max-[560px]:grid-cols-[minmax(0,1fr)_auto_auto]"
      >
        <Label class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground">
          Key
          <Input
            v-model="variable.key"
            class="h-8 font-mono text-xs uppercase"
            autocomplete="off"
            required
          />
        </Label>
        <Label
          class="grid min-w-0 gap-1.5 text-[11px] text-muted-foreground max-[560px]:col-span-3"
        >
          Value
          <Input
            v-model="variable.value"
            class="h-8 font-mono text-xs"
            :type="variable.is_secret && !props.showSecretValues ? 'password' : 'text'"
            :placeholder="
              variable.is_secret && variable.is_set
                ? 'Stored securely; leave blank to keep'
                : 'Enter value'
            "
            autocomplete="off"
            :required="!variable.is_secret || !variable.is_set"
          />
        </Label>
        <div class="grid gap-1.5 text-[11px] text-muted-foreground">
          Secret
          <Switch
            :model-value="variable.is_secret"
            :aria-label="'Mark ' + (variable.key || 'variable') + ' secret'"
            @update:model-value="emit('updateSecret', index, $event)"
          />
        </div>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button
              size="icon"
              type="button"
              variant="outline"
              :aria-label="'Remove ' + (variable.key || 'variable')"
              @click="emit('removeVariable', index)"
              ><Trash2 :stroke-width="1.5"
            /></Button>
          </TooltipTrigger>
          <TooltipContent>Remove variable</TooltipContent>
        </Tooltip>
      </div>
    </div>
    <div v-else class="grid gap-1.5 py-4">
      <div class="flex items-center gap-2">
        <LockKeyhole
          v-if="props.activeEnvironmentKind === 'secrets'"
          class="size-4 text-muted-foreground"
          :stroke-width="1.5"
        />
        <p class="text-sm font-medium">
          {{
            props.activeEnvironmentKind === "secrets"
              ? "No service secrets"
              : "No service variables"
          }}
        </p>
      </div>
      <p class="max-w-[56ch] text-xs leading-5 text-muted-foreground">
        {{
          props.activeEnvironmentKind === "secrets"
            ? "Service secrets override project secrets. Stored values stay masked; leave one blank to keep its current value."
            : "Add a service-specific value when it needs to override a project variable."
        }}
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <Button size="sm" type="button" variant="outline" @click="emit('addVariable', false)"
        ><Plus data-icon="inline-start" :stroke-width="1.5" />Add variable</Button
      >
      <Button size="sm" type="button" variant="outline" @click="emit('addVariable', true)"
        ><LockKeyhole data-icon="inline-start" :stroke-width="1.5" />Add secret</Button
      >
    </div>
  </fieldset>
</template>
