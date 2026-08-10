<script setup lang="ts">
import { Server } from "@lucide/vue";
import { onMounted, shallowRef } from "vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { apiListRemoteServers, type RemoteServerSummary } from "@/lib/api";
import { cn } from "@/lib/utils";

const props = withDefaults(
  defineProps<{
    modelValue: string;
    disabled?: boolean;
    class?: string;
  }>(),
  { disabled: false },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  change: [server: RemoteServerSummary | null];
}>();

const servers = shallowRef<RemoteServerSummary[]>([]);
const loading = shallowRef(true);

async function loadServers() {
  loading.value = true;
  const result = await apiListRemoteServers();
  if (result.success) servers.value = result.data;
  loading.value = false;
}

function selectDestination(value: string) {
  emit("update:modelValue", value);
  emit("change", servers.value.find((server) => server.id === value) ?? null);
}

onMounted(() => void loadServers());
</script>

<template>
  <Select :model-value="props.modelValue" @update:model-value="selectDestination">
    <SelectTrigger :class="cn('min-w-56', props.class)" :disabled="props.disabled">
      <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
      <SelectValue placeholder="This Ignitify host" />
    </SelectTrigger>
    <SelectContent>
      <SelectItem value="local">
        <span class="grid gap-0.5">
          <span>This Ignitify host</span>
          <span class="font-mono text-[10px] text-muted-foreground">Local runtime</span>
        </span>
      </SelectItem>
      <SelectItem v-for="server in servers" :key="server.id" :value="server.id">
        <span class="grid gap-0.5">
          <span>{{ server.name }}</span>
          <span class="font-mono text-[10px] text-muted-foreground">
            {{ server.username }}@{{ server.host }}:{{ server.port }}
            <span v-if="server.agent" class="ml-1">· agent {{ server.agent.status }}</span>
          </span>
        </span>
      </SelectItem>
      <div v-if="loading" class="px-2 py-1.5 text-xs text-muted-foreground">
        Loading destinations...
      </div>
      <div v-else-if="!servers.length" class="px-2 py-1.5 text-xs text-muted-foreground">
        No remote destinations configured
      </div>
    </SelectContent>
  </Select>
</template>
