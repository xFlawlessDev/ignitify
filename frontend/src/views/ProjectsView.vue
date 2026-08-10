<script setup lang="ts">
import {
  Box,
  ChevronLeft,
  ChevronRight,
  KeyRound,
  LayoutGrid,
  List as ListIcon,
  Plus,
  RefreshCw,
  Users,
} from "@lucide/vue";
import { computed, onMounted, shallowRef, watch } from "vue";
import { toast } from "vue-sonner";
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
const PROJECTS_PER_PAGE = 6;
const currentPage = shallowRef(1);
const viewMode = shallowRef<"list" | "catalog">("catalog");
const pageCount = computed(() => Math.max(1, Math.ceil(projectCount.value / PROJECTS_PER_PAGE)));
const visibleProjects = computed(() => {
  const start = (currentPage.value - 1) * PROJECTS_PER_PAGE;
  return data.value.slice(start, start + PROJECTS_PER_PAGE);
});
const firstVisibleProject = computed(() =>
  projectCount.value === 0 ? 0 : (currentPage.value - 1) * PROJECTS_PER_PAGE + 1,
);
const lastVisibleProject = computed(() =>
  Math.min(currentPage.value * PROJECTS_PER_PAGE, projectCount.value),
);

watch(
  pageCount,
  (count) => {
    if (currentPage.value > count) currentPage.value = count;
  },
  { immediate: true },
);

function setViewMode(mode: "list" | "catalog") {
  viewMode.value = mode;
  currentPage.value = 1;
}

function goToPreviousPage() {
  currentPage.value = Math.max(1, currentPage.value - 1);
}

function goToNextPage() {
  currentPage.value = Math.min(pageCount.value, currentPage.value + 1);
}

async function createProject(name: string) {
  const project = await create({ name });
  if (!project) {
    toast.error("Could not create project", {
      description: error.value ?? "Try again in a moment.",
    });
    return;
  }
  createOpen.value = false;
  toast.success("Project created", { description: `${project.name} is ready to configure.` });
  await router.push({ name: "ProjectDetail", params: { projectId: project.id } });
}

async function loadProjects(showSuccess = false) {
  await load();
  if (error.value) {
    toast.error("Projects unavailable", { description: error.value });
    return;
  }
  if (showSuccess) toast.success("Projects refreshed");
}

onMounted(() => void loadProjects());
</script>

<template>
  <div class="app-page">
    <header class="app-page-header lg:grid-cols-[minmax(0,1fr)_auto] lg:items-end">
      <div>
        <p class="ui-label">Workspace</p>
        <h1 class="mt-2 text-3xl leading-none font-normal">Projects</h1>
        <p class="mt-2 max-w-[56ch] text-sm leading-5 text-muted-foreground">
          Organize deployment services, shared environment values, and release history by product.
        </p>
      </div>
      <div class="flex w-full items-center gap-2 sm:w-auto">
        <Button class="order-1 w-full sm:order-none sm:w-auto" @click="createOpen = true">
          <Plus class="size-4" :stroke-width="1.5" />
          New project
        </Button>
        <Button
          variant="ghost"
          class="grid size-9 shrink-0 place-items-center rounded-[3px] border border-border bg-card text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          type="button"
          aria-label="Refresh projects"
          title="Refresh projects"
          :disabled="loading"
          @click="loadProjects(true)"
        >
          <RefreshCw class="size-4" :class="loading ? 'animate-spin' : ''" :stroke-width="1.5" />
        </Button>
      </div>
    </header>

    <section
      class="mt-6 app-surface grid divide-y divide-border sm:grid-cols-3 sm:divide-x sm:divide-y-0"
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

    <section v-if="loading" class="mt-6 app-surface" role="status" aria-label="Loading projects">
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
    <section v-else-if="data.length === 0" class="mt-6 app-surface px-5 py-8">
      <p class="text-sm font-medium">No projects yet</p>
      <p class="mt-1 text-xs text-muted-foreground">
        Create project to get production environment.
      </p>
    </section>
    <section v-else class="mt-6 grid gap-4">
      <div class="flex items-end justify-between gap-4">
        <div>
          <p class="ui-label">Workspace inventory</p>
          <h2 class="mt-2 text-base font-medium">Your projects</h2>
        </div>
        <div class="flex items-center gap-3">
          <div
            class="inline-flex items-center gap-0.5 rounded-sm border border-border bg-muted p-0.5"
            role="group"
            aria-label="Project view"
          >
            <Button
              variant="ghost"
              class="grid size-7 place-items-center rounded-[2px] transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
              :class="
                viewMode === 'list'
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:bg-background/70 hover:text-foreground'
              "
              type="button"
              aria-label="List view"
              title="List view"
              :aria-pressed="viewMode === 'list'"
              @click="setViewMode('list')"
            >
              <ListIcon class="size-4" :stroke-width="1.5" />
            </Button>
            <Button
              variant="ghost"
              class="grid size-7 place-items-center rounded-[2px] transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
              :class="
                viewMode === 'catalog'
                  ? 'bg-background text-foreground'
                  : 'text-muted-foreground hover:bg-background/70 hover:text-foreground'
              "
              type="button"
              aria-label="Catalog view"
              title="Catalog view"
              :aria-pressed="viewMode === 'catalog'"
              @click="setViewMode('catalog')"
            >
              <LayoutGrid class="size-4" :stroke-width="1.5" />
            </Button>
          </div>
          <span class="font-mono text-[11px] text-muted-foreground">{{ projectCount }} total</span>
        </div>
      </div>
      <ProjectList :projects="visibleProjects" :view="viewMode" />
      <nav
        v-if="pageCount > 1"
        class="app-surface flex items-center justify-between gap-4 px-4 py-3 max-[640px]:items-start max-[640px]:flex-col"
        aria-label="Project pagination"
      >
        <p class="text-xs text-muted-foreground" aria-live="polite">
          Showing {{ firstVisibleProject }}–{{ lastVisibleProject }} of {{ projectCount }} projects
        </p>
        <div class="flex items-center gap-2">
          <Button
            size="icon-sm"
            variant="outline"
            :disabled="currentPage === 1"
            aria-label="Previous page"
            @click="goToPreviousPage"
          >
            <ChevronLeft class="size-4" :stroke-width="1.5" />
          </Button>
          <span class="min-w-20 text-center font-mono text-xs text-muted-foreground">
            Page {{ currentPage }} of {{ pageCount }}
          </span>
          <Button
            size="icon-sm"
            variant="outline"
            :disabled="currentPage === pageCount"
            aria-label="Next page"
            @click="goToNextPage"
          >
            <ChevronRight class="size-4" :stroke-width="1.5" />
          </Button>
        </div>
      </nav>
    </section>

    <ProjectCreateDialog v-model:open="createOpen" @create="createProject" />
  </div>
</template>
