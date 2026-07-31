<script setup lang="ts">
import { shallowRef } from "vue";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

defineProps<{ error?: string | null }>();

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{ create: [name: string] }>();

const name = shallowRef("");

function submit() {
  const value = name.value.trim();
  if (!value) return;
  emit("create", value);
}
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="rounded-md shadow-none sm:max-w-md">
      <DialogHeader>
        <DialogTitle>New project</DialogTitle>
        <DialogDescription>Projects hold one immutable production environment.</DialogDescription>
      </DialogHeader>
      <form class="grid gap-2" @submit.prevent="submit">
        <Label for="project-name">Project name</Label>
        <Input id="project-name" v-model="name" maxlength="100" autocomplete="off" />
        <p v-if="error" class="text-xs text-destructive" role="alert">{{ error }}</p>
        <DialogFooter class="mt-2">
          <Button type="submit">Create project</Button>
        </DialogFooter>
      </form>
    </DialogContent>
  </Dialog>
</template>
