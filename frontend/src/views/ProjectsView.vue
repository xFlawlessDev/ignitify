<script setup lang="ts">
import { Box, KeyRound, Plus, RefreshCw, Users } from "@lucide/vue";
import { computed, onMounted, shallowRef } from "vue";
import { useRouter } from "vue-router";
import ProjectCreateDialog from "@/components/project/ProjectCreateDialog.vue";
import ProjectList from "@/components/project/ProjectList.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useProjects } from "@/composables/useProjects";

const router = useRouter();
const { create, data, error, load, loading } = useProjects();
const createOpen = shallowRef(false);
const projectCount = computed(() => data.value.length);
const ownerCount = computed(() => data.value.filter((project) => project.role === "owner").length);
const environmentCount = computed(
  () => data.value.filter((project) => project.default_environment.is_default).length,
);

async function createProject(name: string) {
  const project = await create({ name });
  if (!project) return;
  createOpen.value = false;
  await router.push({ name: "ProjectDetail", params: { projectId: project.id } });
}

onMounted(load);
</script>

<template>
  <div class="w-full max-w-[1200px]">
    <header
      class="flex items-end justify-between gap-5 border-b border-border pb-[25px] max-[640px]:items-start max-[640px]:flex-col"
    >
      <div>
        <p class="ui-label">Workspace</p>
        <h1 class="mt-2.5 text-[30px] leading-none font-medium">Projects</h1>
        <p class="mt-2.5 max-w-[56ch] text-[13px] leading-5 text-muted-foreground">
          Organize deployment services, shared environment values, and release history by product.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <Button class="order-1 w-full sm:order-none sm:w-auto" @click="createOpen = true">
          <Plus class="size-4" :stroke-width="1.5" />
          New project
        </Button>
        <button
          class="grid size-9 shrink-0 place-items-center rounded-[3px] border border-border bg-card text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          type="button"
          aria-label="Refresh projects"
          title="Refresh projects"
          :disabled="loading"
          @click="load"
        >
          <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        </button>
      </div>
    </header>

    <section
      class="mt-[22px] grid overflow-hidden divide-y divide-border border border-border bg-card sm:grid-cols-3 sm:divide-x sm:divide-y-0"
      aria-label="Project summary"
    >
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <Box class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Projects</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ projectCount }}</strong>
        </div>
      </div>
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <KeyRound class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Environments</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ environmentCount }}</strong>
        </div>
      </div>
      <div class="flex min-h-[86px] items-center gap-3 px-5 py-4">
        <Users class="size-4 text-muted-foreground" :stroke-width="1.5" />
        <div class="grid gap-1">
          <span class="ui-label">Owned by you</span>
          <strong class="font-mono text-lg font-medium tabular-nums">{{ ownerCount }}</strong>
        </div>
      </div>
    </section>

    <section
      v-if="loading"
      class="mt-[22px] border border-border bg-card"
      role="status"
      aria-label="Loading projects"
    >
      <div
        v-for="index in 4"
        :key="index"
        class="flex min-h-[78px] items-center gap-3.5 border-b border-border px-4 py-3 last:border-b-0 sm:px-[18px]"
      >
        <Skeleton class="size-[30px] shrink-0 rounded-[4px]" />
        <div class="grid min-w-0 flex-1 gap-2">
          <Skeleton class="h-3 w-40 max-w-full" />
          <Skeleton class="h-2.5 w-28 max-w-full" />
        </div>
        <Skeleton class="size-4 shrink-0" />
      </div>
    </section>
    <section
      v-else-if="error"
      class="mt-[22px] border border-destructive/40 bg-card px-5 py-8"
      role="alert"
    >
      <p class="text-sm text-destructive">{{ error }}</p>
      <Button class="mt-4" variant="outline" size="sm" @click="load">
        <RefreshCw class="size-4" :stroke-width="1.5" />
        Retry
      </Button>
    </section>
    <section v-else-if="data.length === 0" class="mt-[22px] border border-border bg-card px-5 py-8">
      <p class="text-sm font-medium">No projects yet</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Create project to get production environment.
      </p>
    </section>
    <section v-else class="mt-[22px] grid gap-3">
      <div class="flex items-end justify-between gap-4">
        <div>
          <p class="ui-label">Workspace inventory</p>
          <h2 class="mt-2 text-base font-medium">Your projects</h2>
        </div>
        <span class="font-mono text-[11px] text-muted-foreground">{{ projectCount }} total</span>
      </div>
      <ProjectList :projects="data" />
    </section>

    <ProjectCreateDialog v-model:open="createOpen" :error="error" @create="createProject" />
  </div>
</template>
