<script setup lang="ts">
import { Check, Copy, Server, Upload } from "@lucide/vue";
import { Button } from "@/components/ui/button";
import { DialogClose, DialogFooter } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";

type SecretInputMode = "file" | "text";

interface RemoteServerForm {
  name: string;
  host: string;
  port: number;
  username: string;
  deployPath: string;
  privateKeyText: string;
  publicKeyText: string;
  knownHosts: string;
  isDefault: boolean;
}

defineProps<{
  copiedGuideCommand: string | null;
  form: RemoteServerForm;
  formError: string;
  linuxGuideCommands: Record<"generate" | "install" | "hostKey", string>;
  privateKeyFile: File | null;
  privateKeyInputKey: number;
  privateKeyMode: SecretInputMode;
  publicKeyFile: File | null;
  publicKeyInputKey: number;
  publicKeyMode: SecretInputMode;
  saving: boolean;
  showValidation: boolean;
}>();

const emit = defineEmits<{
  copyGuideCommand: [command: string];
  save: [];
  updatePrivateKey: [event: Event];
  updatePublicKey: [event: Event];
  updatePrivateKeyMode: [mode: SecretInputMode];
  updatePublicKeyMode: [mode: SecretInputMode];
}>();
</script>

<template>
  <details class="border-y border-border py-3 text-xs">
    <summary class="cursor-pointer font-medium text-foreground">Linux SSH setup guide</summary>
    <div
      class="mt-3 grid max-h-[min(42vh,320px)] gap-3 overflow-y-auto pr-1 text-[11px] leading-5 text-muted-foreground"
    >
      <ol class="grid gap-3 pl-4">
        <li>
          <span class="font-medium text-foreground">Create a deploy key</span> on the Ignitify host
          or your workstation. This creates the private key and matching
          <code class="font-mono text-foreground">.pub</code> file. Keep the passphrase empty
          because automated SSH checks cannot prompt for one.
          <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
            <pre
              class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
            ><code>{{ linuxGuideCommands.generate }}</code></pre>
            <Button
              variant="ghost"
              size="icon"
              class="mt-0.5 size-7 shrink-0 rounded-[4px]"
              type="button"
              :aria-label="
                copiedGuideCommand === linuxGuideCommands.generate
                  ? 'Copied'
                  : 'Copy key generation command'
              "
              :title="
                copiedGuideCommand === linuxGuideCommands.generate ? 'Copied' : 'Copy command'
              "
              @click="emit('copyGuideCommand', linuxGuideCommands.generate)"
            >
              <Check
                v-if="copiedGuideCommand === linuxGuideCommands.generate"
                class="size-3.5 text-metric-green"
                :stroke-width="1.8"
              />
              <Copy v-else class="size-3.5" :stroke-width="1.5" />
            </Button>
          </div>
        </li>
        <li>
          <span class="font-medium text-foreground">Install the public key</span> on the remote
          Linux account that will run deployments.
          <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
            <pre
              class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
            ><code>{{ linuxGuideCommands.install }}</code></pre>
            <Button
              variant="ghost"
              size="icon"
              class="mt-0.5 size-7 shrink-0 rounded-[4px]"
              type="button"
              :aria-label="
                copiedGuideCommand === linuxGuideCommands.install
                  ? 'Copied'
                  : 'Copy public key installation command'
              "
              :title="copiedGuideCommand === linuxGuideCommands.install ? 'Copied' : 'Copy command'"
              @click="emit('copyGuideCommand', linuxGuideCommands.install)"
            >
              <Check
                v-if="copiedGuideCommand === linuxGuideCommands.install"
                class="size-3.5 text-metric-green"
                :stroke-width="1.8"
              />
              <Copy v-else class="size-3.5" :stroke-width="1.5" />
            </Button>
          </div>
        </li>
        <li>
          <span class="font-medium text-foreground">Pin the server host key</span> before
          connecting. Verify the fingerprint with your provider, then paste this output in the
          <code class="font-mono text-foreground">known_hosts</code> field.
          <div class="mt-1.5 flex min-w-0 items-start gap-1.5">
            <pre
              class="min-w-0 flex-1 overflow-x-auto rounded-[4px] border border-border bg-muted/50 p-2 font-mono text-[10px] leading-4 text-foreground"
            ><code>{{ linuxGuideCommands.hostKey }}</code></pre>
            <Button
              variant="ghost"
              size="icon"
              class="mt-0.5 size-7 shrink-0 rounded-[4px]"
              type="button"
              :aria-label="
                copiedGuideCommand === linuxGuideCommands.hostKey
                  ? 'Copied'
                  : 'Copy host key command'
              "
              :title="copiedGuideCommand === linuxGuideCommands.hostKey ? 'Copied' : 'Copy command'"
              @click="emit('copyGuideCommand', linuxGuideCommands.hostKey)"
            >
              <Check
                v-if="copiedGuideCommand === linuxGuideCommands.hostKey"
                class="size-3.5 text-metric-green"
                :stroke-width="1.8"
              />
              <Copy v-else class="size-3.5" :stroke-width="1.5" />
            </Button>
          </div>
        </li>
      </ol>
      <div class="grid gap-1 border-l-2 border-border pl-3">
        <p class="font-medium text-foreground">Field mapping</p>
        <p>
          <code class="font-mono text-foreground">Private key</code>: file without
          <code class="font-mono text-foreground">.pub</code> or its full private-key text.
          <code class="font-mono text-foreground">Public key</code>: matching
          <code class="font-mono text-foreground">.pub</code> line.
          <code class="font-mono text-foreground">known_hosts</code>: remote host key; it is
          different from the client public key.
        </p>
      </div>
    </div>
  </details>

  <form class="grid gap-4" @submit.prevent="emit('save')">
    <div class="grid gap-4 sm:grid-cols-2">
      <Label class="grid gap-2 text-xs font-medium" for="remote-server-name">
        Server name
        <Input
          id="remote-server-name"
          v-model="form.name"
          class="rounded-[3px]"
          autocomplete="off"
        />
      </Label>
      <Label class="grid gap-2 text-xs font-medium" for="remote-server-host">
        Hostname or IP
        <Input
          id="remote-server-host"
          v-model="form.host"
          class="rounded-[3px] font-mono text-xs"
          placeholder="deploy.example.com"
          autocomplete="off"
        />
      </Label>
    </div>
    <div class="grid gap-4 sm:grid-cols-[110px_minmax(0,1fr)_minmax(0,1fr)]">
      <Label class="grid gap-2 text-xs font-medium" for="remote-server-port">
        SSH port
        <Input
          id="remote-server-port"
          v-model.number="form.port"
          class="rounded-[3px] font-mono text-xs"
          type="number"
          min="1"
          max="65535"
          inputmode="numeric"
        />
      </Label>
      <Label class="grid gap-2 text-xs font-medium" for="remote-server-user">
        SSH user
        <Input
          id="remote-server-user"
          v-model="form.username"
          class="rounded-[3px] font-mono text-xs"
          autocomplete="username"
        />
      </Label>
      <Label class="grid gap-2 text-xs font-medium" for="remote-server-path">
        Deploy path
        <Input
          id="remote-server-path"
          v-model="form.deployPath"
          class="rounded-[3px] font-mono text-xs"
          placeholder="/srv/ignitify"
          autocomplete="off"
        />
      </Label>
    </div>

    <div class="grid gap-4 border-t border-border pt-4 sm:grid-cols-2">
      <div class="grid content-start gap-2">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-xs font-medium">SSH private key</Label>
          <div class="inline-flex rounded-[4px] border border-border p-0.5" role="tablist">
            <button
              class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
              :class="privateKeyMode === 'file' ? 'bg-muted text-foreground' : ''"
              type="button"
              role="tab"
              :aria-selected="privateKeyMode === 'file'"
              @click="emit('updatePrivateKeyMode', 'file')"
            >
              File
            </button>
            <button
              class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
              :class="privateKeyMode === 'text' ? 'bg-muted text-foreground' : ''"
              type="button"
              role="tab"
              :aria-selected="privateKeyMode === 'text'"
              @click="emit('updatePrivateKeyMode', 'text')"
            >
              Text
            </button>
          </div>
        </div>
        <template v-if="privateKeyMode === 'file'">
          <Label
            for="remote-server-key"
            class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
          >
            <Upload class="size-4 shrink-0" :stroke-width="1.5" />
            <span class="truncate">{{ privateKeyFile?.name ?? "Keep current key" }}</span>
          </Label>
          <input
            :key="privateKeyInputKey"
            id="remote-server-key"
            class="sr-only"
            type="file"
            accept="*/*"
            @change="emit('updatePrivateKey', $event)"
          />
        </template>
        <Textarea
          v-else
          v-model="form.privateKeyText"
          class="min-h-[112px] rounded-[3px] font-mono text-[10px] leading-4"
          placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="grid content-start gap-2">
        <div class="flex items-center justify-between gap-3">
          <Label class="text-xs font-medium">SSH public key</Label>
          <div class="inline-flex rounded-[4px] border border-border p-0.5" role="tablist">
            <button
              class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
              :class="publicKeyMode === 'file' ? 'bg-muted text-foreground' : ''"
              type="button"
              role="tab"
              :aria-selected="publicKeyMode === 'file'"
              @click="emit('updatePublicKeyMode', 'file')"
            >
              File
            </button>
            <button
              class="rounded-[3px] px-2 py-1 font-mono text-[10px] text-muted-foreground transition-colors hover:bg-muted"
              :class="publicKeyMode === 'text' ? 'bg-muted text-foreground' : ''"
              type="button"
              role="tab"
              :aria-selected="publicKeyMode === 'text'"
              @click="emit('updatePublicKeyMode', 'text')"
            >
              Text
            </button>
          </div>
        </div>
        <template v-if="publicKeyMode === 'file'">
          <Label
            for="remote-server-public-key"
            class="flex h-9 cursor-pointer items-center gap-2 rounded-[3px] border border-input px-3 text-xs text-muted-foreground hover:bg-muted"
          >
            <Upload class="size-4 shrink-0" :stroke-width="1.5" />
            <span class="truncate">{{ publicKeyFile?.name ?? "Keep current key" }}</span>
          </Label>
          <input
            :key="publicKeyInputKey"
            id="remote-server-public-key"
            class="sr-only"
            type="file"
            accept=".pub,.txt"
            @change="emit('updatePublicKey', $event)"
          />
        </template>
        <Textarea
          v-else
          v-model="form.publicKeyText"
          class="min-h-[112px] rounded-[3px] font-mono text-[10px] leading-4"
          placeholder="ssh-ed25519 AAAAC3... user@host"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <Label class="grid gap-2 text-xs font-medium sm:col-span-2" for="remote-server-known-hosts">
        known_hosts (server host key)
        <Textarea
          id="remote-server-known-hosts"
          v-model="form.knownHosts"
          class="min-h-[88px] rounded-[3px] font-mono text-[11px] leading-4"
          placeholder="Keep current host trust record"
          autocomplete="off"
        />
      </Label>
    </div>

    <div class="flex items-center justify-between gap-3 border-t border-border pt-4">
      <div>
        <p class="text-xs font-medium">Use as default destination</p>
        <p class="mt-1 text-[11px] text-muted-foreground">
          Marks the primary target when a remote runner is attached.
        </p>
      </div>
      <Switch :model-value="form.isDefault" @update:model-value="form.isDefault = $event" />
    </div>
    <p v-if="showValidation && formError" class="text-[11px] text-destructive" role="alert">
      {{ formError }}
    </p>
    <DialogFooter>
      <DialogClose as-child><Button variant="outline" type="button">Cancel</Button></DialogClose>
      <Button type="submit" :disabled="saving"
        ><Server class="size-4" :stroke-width="1.5" />{{
          saving ? "Saving" : "Save changes"
        }}</Button
      >
    </DialogFooter>
  </form>
</template>
