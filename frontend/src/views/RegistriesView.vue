<script setup lang="ts">
import { CircleAlert, Plus, RefreshCw, Server, Trash2 } from "@lucide/vue";
import { onMounted, shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useAuthStore } from "@/stores/auth";
import { apiCreateRegistry, apiDeleteRegistry, apiListRegistries } from "@/lib/api/registries";
import type { RegistrySummary } from "@/lib/types";

const auth = useAuthStore();
const registries = shallowRef<RegistrySummary[]>([]);
const error = shallowRef<string | null>(null);
const loading = shallowRef(false);
const submitting = shallowRef(false);
const name = shallowRef("");
const endpoint = shallowRef("");
const username = shallowRef("");
const credential = shallowRef("");
const removing = shallowRef<RegistrySummary | null>(null);
const confirmName = shallowRef("");

async function load() {
  loading.value = true;
  error.value = null;
  const result = await apiListRegistries();
  if (result.success) registries.value = result.data;
  else error.value = result.error ?? "Could not load registries";
  loading.value = false;
}

async function create() {
  if (!name.value.trim() || !endpoint.value.trim()) return;
  submitting.value = true;
  error.value = null;
  const result = await apiCreateRegistry({
    name: name.value.trim(),
    endpoint: endpoint.value.trim(),
    ...(username.value ? { username: username.value.trim() } : {}),
    ...(credential.value ? { credential: credential.value } : {}),
  });
  if (result.success) {
    registries.value = [result.data, ...registries.value];
    name.value = "";
    endpoint.value = "";
    username.value = "";
    credential.value = "";
  } else error.value = result.error ?? "Could not create registry";
  submitting.value = false;
}

function startRemove(registry: RegistrySummary) {
  removing.value = registry;
  confirmName.value = "";
}

async function remove() {
  const registry = removing.value;
  if (!registry || confirmName.value !== registry.name) return;
  submitting.value = true;
  const result = await apiDeleteRegistry(registry.id, confirmName.value);
  if (result.success) registries.value = registries.value.filter((item) => item.id !== registry.id);
  else error.value = result.error ?? "Could not remove registry";
  submitting.value = false;
  removing.value = null;
  confirmName.value = "";
}

onMounted(load);
</script>

<template>
  <div class="max-w-[1160px]">
    <header
      class="flex items-end justify-between gap-6 border-b border-border pb-[25px] max-[620px]:items-start max-[620px]:flex-col"
    >
      <div>
        <p class="ui-label">Admin</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Registries</h1>
        <p class="mt-2.5 text-[13px] text-muted-foreground">
          Private image sources for deployment pulls.
        </p>
      </div>
      <Server class="size-5 text-muted-foreground" :stroke-width="1.5" />
    </header>
    <section
      v-if="!auth.isAdmin"
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-6 text-sm text-destructive"
      role="alert"
    >
      Registry management requires administrator access.
    </section>
    <template v-else>
      <section
        v-if="error"
        class="mt-4 flex items-center justify-between gap-4 border border-destructive/40 bg-card px-5 py-4 text-sm text-destructive"
        role="alert"
      >
        <span class="flex items-center gap-2"
          ><CircleAlert class="size-4" :stroke-width="1.5" />{{ error }}</span
        >
        <Button size="sm" variant="outline" @click="load"
          ><RefreshCw class="size-4" :stroke-width="1.5" /> Retry</Button
        >
      </section>
      <section class="mt-[22px] border border-border bg-card">
        <div class="border-b border-border px-5 py-4">
          <p class="ui-label">Add source</p>
          <h2 class="mt-2 text-base font-medium">Registry connection</h2>
        </div>
        <div class="grid gap-3 p-5 md:grid-cols-[1fr_1.5fr_1fr_1fr_auto]">
          <Input v-model="name" placeholder="Name" aria-label="Registry name" />
          <Input
            v-model="endpoint"
            type="url"
            placeholder="https://registry.example.com"
            aria-label="Registry endpoint"
          />
          <Input v-model="username" placeholder="Username" aria-label="Registry username" />
          <Input
            v-model="credential"
            type="password"
            placeholder="Credential"
            aria-label="Registry credential"
          />
          <Button :disabled="submitting || !name.trim() || !endpoint.trim()" @click="create"
            ><Plus class="size-4" :stroke-width="1.5" /> Add</Button
          >
        </div>
      </section>
      <section class="mt-4 border border-border bg-card">
        <div v-if="loading" class="px-5 py-8 text-sm text-muted-foreground" role="status">
          Loading registries...
        </div>
        <div v-else-if="!registries.length" class="px-5 py-8 text-sm text-muted-foreground">
          No private registries configured.
        </div>
        <div v-else class="divide-y divide-border">
          <div
            v-for="registry in registries"
            :key="registry.id"
            class="flex items-center justify-between gap-4 px-5 py-4"
          >
            <div class="min-w-0">
              <p class="text-sm font-medium">{{ registry.name }}</p>
              <p class="mt-1 truncate font-mono text-[11px] text-muted-foreground">
                {{ registry.endpoint }}
              </p>
              <p class="mt-1 text-[11px] text-muted-foreground">
                {{ registry.credential_configured ? "Credential configured" : "Anonymous pull" }}
              </p>
            </div>
            <Button
              size="icon"
              variant="ghost"
              :disabled="submitting"
              :aria-label="`Remove ${registry.name}`"
              @click="startRemove(registry)"
              ><Trash2 class="size-4" :stroke-width="1.5"
            /></Button>
          </div>
        </div>
      </section>
      <section
        v-if="removing"
        class="mt-4 flex items-center gap-3 border border-destructive/40 bg-card p-5"
      >
        <Input
          v-model="confirmName"
          :placeholder="`Type ${removing.name} to remove`"
          :aria-label="`Confirm removal of ${removing.name}`"
        />
        <Button
          variant="destructive"
          :disabled="submitting || confirmName !== removing.name"
          @click="remove"
          >Remove</Button
        >
        <Button variant="outline" :disabled="submitting" @click="removing = null">Cancel</Button>
      </section>
    </template>
  </div>
</template>
