<script setup lang="ts">
import { Plus, RefreshCw } from "@lucide/vue";
import { onMounted, shallowRef } from "vue";
import { useRouter } from "vue-router";
import ProjectCreateDialog from "@/components/project/ProjectCreateDialog.vue";
import ProjectList from "@/components/project/ProjectList.vue";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { useProjects } from "@/composables/useProjects";

const router = useRouter();
const { create, data, error, load, loading } = useProjects();
const createOpen = shallowRef(false);

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
        <h1 class="mt-3 text-[30px] leading-none font-normal">Projects</h1>
        <p class="mt-2 text-xs text-muted-foreground">
          Deployments grouped by product and environment.
        </p>
      </div>
      <Button class="w-full sm:w-auto" @click="createOpen = true">
        <Plus class="size-4" :stroke-width="1.5" />
        New project
      </Button>
    </header>

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
    <ProjectList v-else class="mt-[22px]" :projects="data" />

    <ProjectCreateDialog v-model:open="createOpen" :error="error" @create="createProject" />
  </div>
</template>
