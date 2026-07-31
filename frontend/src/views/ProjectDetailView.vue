<script setup lang="ts">
import { ArrowLeft, Box, Pencil, RefreshCw } from "@lucide/vue";
import { shallowRef, watch } from "vue";
import { RouterLink, useRoute } from "vue-router";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useProject } from "@/composables/useProject";

const route = useRoute();
const { data, error, load: fetchProject, loading, update } = useProject();
const activeTab = shallowRef("overview");
const editName = shallowRef("");

function load(projectId: string) {
  void fetchProject(projectId).then(() => {
    editName.value = data.value?.name ?? "";
  });
}

async function renameProject() {
  if (!editName.value.trim()) return;
  await update({ name: editName.value });
}

watch(() => String(route.params.projectId), load, { immediate: true });
</script>

<template>
  <div class="max-w-[1160px]">
    <RouterLink
      class="inline-flex items-center gap-1.5 text-xs text-muted-foreground hover:text-foreground"
      to="/projects"
    >
      <ArrowLeft :size="15" :stroke-width="1.5" />
      Projects
    </RouterLink>

    <p
      v-if="loading"
      class="mt-[22px] border border-border bg-card px-5 py-8 text-sm text-muted-foreground"
      role="status"
    >
      Loading project...
    </p>
    <section
      v-else-if="error"
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-8"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button
        class="mt-4"
        variant="outline"
        size="sm"
        @click="load(String(route.params.projectId))"
      >
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Retry
      </Button>
    </section>
    <template v-else-if="data">
      <header
        class="mt-[22px] flex items-center justify-between gap-6 border-b border-border pb-[25px] max-[620px]:items-start max-[620px]:flex-col"
      >
        <div class="flex min-w-0 items-center gap-[13px]">
          <div
            class="grid size-11 shrink-0 place-items-center rounded-[5px] border border-border bg-muted text-muted-foreground"
          >
            <Box :size="20" :stroke-width="1.5" />
          </div>
          <div>
            <h1 class="m-0 truncate text-[29px] leading-none font-normal">
              {{ data.name }}
            </h1>
            <p class="mt-2 text-xs text-muted-foreground">
              {{ data.default_environment.name }} environment
            </p>
          </div>
        </div>
      </header>

      <nav
        class="mt-[25px] flex h-[39px] gap-1 overflow-x-auto border-b border-border"
        aria-label="Project sections"
      >
        <button
          v-for="tab in ['overview', 'services', 'deployments', 'settings']"
          :key="tab"
          class="h-[39px] flex-none border-b-2 border-b-transparent px-2.5 text-xs text-muted-foreground capitalize hover:text-foreground"
          :class="activeTab === tab ? 'border-b-[var(--status-live)] text-foreground' : ''"
          type="button"
          :aria-current="activeTab === tab ? 'page' : undefined"
          @click="activeTab = tab"
        >
          {{ tab }}
        </button>
      </nav>

      <section
        v-if="activeTab === 'overview'"
        class="mt-[22px] grid border border-border bg-card sm:grid-cols-2"
      >
        <div class="grid gap-2 border-b border-border p-5 sm:border-r sm:border-b-0">
          <p class="ui-label">Default environment</p>
          <strong class="text-[15px] font-medium">{{ data.default_environment.name }}</strong>
          <span class="text-xs text-muted-foreground">Ready for service configuration.</span>
        </div>
        <div class="grid gap-2 p-5">
          <p class="ui-label">Access</p>
          <strong class="text-[15px] font-medium capitalize">{{ data.role }}</strong>
          <span class="text-xs text-muted-foreground"
            >Project membership controls future resources.</span
          >
        </div>
      </section>

      <section
        v-else-if="activeTab === 'services'"
        class="mt-[22px] border border-border bg-card px-5 py-8"
      >
        <p class="text-sm font-medium">No services configured</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Service configuration arrives in next phase.
        </p>
      </section>

      <section
        v-else-if="activeTab === 'deployments'"
        class="mt-[22px] border border-border bg-card px-5 py-8"
      >
        <p class="text-sm font-medium">No deployments yet</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Deployment history arrives after service configuration.
        </p>
      </section>

      <form
        v-else-if="data.role === 'owner'"
        class="mt-[22px] grid max-w-lg gap-3 border border-border bg-card p-5"
        @submit.prevent="renameProject"
      >
        <div class="flex items-center gap-2">
          <Pencil :size="15" :stroke-width="1.5" class="text-muted-foreground" />
          <h2 class="text-sm font-medium">Project settings</h2>
        </div>
        <label class="grid gap-2 text-xs text-muted-foreground">
          Project name
          <Input v-model="editName" maxlength="100" />
        </label>
        <p v-if="error" class="text-xs text-destructive">{{ error }}</p>
        <Button class="w-fit" type="submit">Save name</Button>
      </form>
      <section v-else class="mt-[22px] border border-border bg-card px-5 py-8">
        <p class="text-sm font-medium">Read-only project</p>
        <p class="mt-1 text-xs text-muted-foreground">
          Your membership role cannot change project settings.
        </p>
      </section>
    </template>
  </div>
</template>
