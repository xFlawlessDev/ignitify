<script setup lang="ts">
import { Plus, RefreshCw } from "@lucide/vue";
import { onMounted, shallowRef } from "vue";
import { useRouter } from "vue-router";
import ProjectCreateDialog from "@/components/project/ProjectCreateDialog.vue";
import ProjectList from "@/components/project/ProjectList.vue";
import { Button } from "@/components/ui/button";
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
  <div class="max-w-[1160px]">
    <header
      class="flex items-end justify-between gap-5 border-b border-border pb-[25px] max-[560px]:items-start max-[560px]:flex-col"
    >
      <div>
        <p class="ui-label">Workspace</p>
        <h1 class="mt-3 text-[30px] leading-none font-normal">Projects</h1>
        <p class="mt-2 text-xs text-muted-foreground">
          Deployments grouped by product and environment.
        </p>
      </div>
      <Button class="max-[560px]:w-full" @click="createOpen = true">
        <Plus class="size-4" :stroke-width="1.5" />
        New project
      </Button>
    </header>

    <p
      v-if="loading"
      class="mt-[22px] border border-border bg-card px-5 py-8 text-sm text-muted-foreground"
      role="status"
    >
      Loading projects...
    </p>
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
