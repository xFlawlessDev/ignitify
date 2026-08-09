<script setup lang="ts">
import { LockKeyhole } from "@lucide/vue";
import { onMounted, shallowRef } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { apiBootstrapStatus } from "@/lib/api";
import { useAuthStore } from "@/stores/auth";

const auth = useAuthStore();
const route = useRoute();
const router = useRouter();
const username = shallowRef("");
const password = shallowRef("");
const bootstrapSecret = shallowRef("");
const bootstrapRequired = shallowRef(false);
const bootstrapEnabled = shallowRef(false);
const error = shallowRef<string | null>(null);
const loading = shallowRef(false);

onMounted(async () => {
  const result = await apiBootstrapStatus();
  if (result.success) {
    bootstrapRequired.value = result.data.required;
    bootstrapEnabled.value = result.data.enabled;
  }
});

async function submit(): Promise<void> {
  if (bootstrapRequired.value && !bootstrapEnabled.value) {
    error.value = "Bootstrap is not configured";
    return;
  }
  loading.value = true;
  error.value = bootstrapRequired.value
    ? await auth.bootstrap(username.value.trim(), password.value, bootstrapSecret.value)
    : await auth.login(username.value.trim(), password.value);
  loading.value = false;
  if (!error.value) {
    await router.replace(
      typeof route.query.redirect === "string" ? route.query.redirect : "/dashboard",
    );
  }
}
</script>

<template>
  <div class="mx-auto grid min-h-[100dvh] max-w-[1200px] place-items-center px-5 py-10 sm:px-6">
    <section class="w-full max-w-md rounded-[10px] border border-border bg-card p-6 sm:p-8">
      <div class="flex items-start justify-between gap-5 border-b border-border pb-6">
        <div>
          <p class="ui-label">Ignitify control plane</p>
          <h1 class="mt-4 text-3xl font-normal leading-tight tracking-normal">
            {{
              bootstrapRequired
                ? bootstrapEnabled
                  ? "Create administrator"
                  : "Bootstrap unavailable"
                : "Sign in"
            }}
          </h1>
        </div>
        <span class="grid size-9 place-items-center rounded-[6px] border border-border bg-muted/30">
          <LockKeyhole class="size-4 text-muted-foreground" stroke-width="1.5" />
        </span>
      </div>

      <form class="mt-7 space-y-5" @submit.prevent="submit">
        <Label class="grid gap-2">
          <span class="ui-label">Username</span>
          <Input v-model="username" autocomplete="username" required />
        </Label>
        <Label v-if="bootstrapRequired && bootstrapEnabled" class="grid gap-2">
          <span class="ui-label">Bootstrap secret</span>
          <Input
            v-model="bootstrapSecret"
            type="password"
            autocomplete="off"
            minlength="32"
            required
          />
        </Label>
        <Label class="grid gap-2">
          <span class="ui-label">Password</span>
          <Input
            v-model="password"
            type="password"
            :autocomplete="bootstrapRequired ? 'new-password' : 'current-password'"
            minlength="8"
            required
          />
        </Label>
        <p
          v-if="error"
          class="border-l-2 border-destructive pl-3 text-sm text-destructive"
          role="alert"
        >
          {{ error }}
        </p>
        <Button class="w-full" :disabled="loading || (bootstrapRequired && !bootstrapEnabled)">
          {{
            loading
              ? "Working..."
              : bootstrapRequired
                ? bootstrapEnabled
                  ? "Create administrator"
                  : "Bootstrap unavailable"
                : "Sign in"
          }}
        </Button>
      </form>
    </section>
  </div>
</template>
