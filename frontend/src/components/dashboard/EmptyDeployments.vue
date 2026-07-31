<script setup lang="ts">
import { ArrowUpRight, Plus } from "@lucide/vue";
import { RouterLink } from "vue-router";
import { Button } from "@/components/ui/button";
</script>

<template>
  <section class="grid overflow-hidden rounded-lg border border-border lg:grid-cols-[1fr_17rem]">
    <div class="min-h-64 p-6 sm:p-8">
      <p class="ui-label">Deployments</p>
      <div class="mt-16 max-w-md">
        <h2 class="text-3xl font-normal leading-tight tracking-normal">Nothing deployed yet.</h2>
        <p class="mt-3 text-sm leading-6 text-muted-foreground">
          Create a project, attach a source, then ship first service from this control plane.
        </p>
        <Button as-child class="mt-6">
          <RouterLink to="/projects">
            <Plus class="size-4" stroke-width="1.5" />
            Create project
          </RouterLink>
        </Button>
      </div>
    </div>
    <div class="border-t border-border bg-[#eeeeee] p-6 text-[#101010] lg:border-t-0 lg:border-l">
      <p class="ui-label !text-[#4d4947]">Control plane</p>
      <p class="mt-20 text-lg font-normal leading-6">Ready for first workload.</p>
      <RouterLink
        to="/projects"
        class="mt-5 inline-flex items-center gap-2 text-sm underline-offset-4 hover:underline"
      >
        Open projects
        <ArrowUpRight class="size-4" stroke-width="1.5" />
      </RouterLink>
    </div>
  </section>
</template>
