import { computed, onMounted, onUnmounted, reactive, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import {
  apiCheckRemoteServer,
  apiCreateRemoteServer,
  apiDeleteRemoteServer,
  apiGetRemoteServerAccess,
  apiInstallRemoteServerAgent,
  apiListRemoteServers,
  apiSetDefaultRemoteServer,
  apiUpdateRemoteServer,
  type RemoteServerInput,
  type RemoteServerSummary,
} from "@/lib/api";

interface ConnectionCheckState {
  serverId: string;
  status: "success" | "error";
  latencyMs?: number;
  message: string;
}

interface RemoteServerAccessSetup {
  server: RemoteServerSummary;
  publicKey: string;
}

type SecretInputMode = "file" | "text";

const linuxGuideCommands = {
  generate: 'ssh-keygen -t ed25519 -N "" -f ./ignitify_deploy -C "ignitify-deploy"',
  install:
    "ssh-copy-id -i ./ignitify_deploy.pub {user}@{host}\nchmod 700 ~/.ssh\nchmod 600 ~/.ssh/authorized_keys",
  hostKey: "ssh-keyscan -t ed25519 {host}",
};

export function useRemoteServers() {
  const { t } = useI18n();
  const servers = shallowRef<RemoteServerSummary[]>([]);
  const selectedServerId = shallowRef<string | null>(null);
  const loading = shallowRef(true);
  const saving = shallowRef(false);
  const removing = shallowRef(false);
  const requestError = shallowRef("");
  const dialogOpen = shallowRef(false);
  const accessDialogOpen = shallowRef(false);
  const deleteDialogOpen = shallowRef(false);
  const serverPendingDeletion = shallowRef<RemoteServerSummary | null>(null);
  const editingId = shallowRef<string | null>(null);
  const privateKeyFile = shallowRef<File | null>(null);
  const privateKeyInputKey = shallowRef(0);
  const privateKeyMode = shallowRef<SecretInputMode>("file");
  const publicKeyFile = shallowRef<File | null>(null);
  const publicKeyInputKey = shallowRef(0);
  const publicKeyMode = shallowRef<SecretInputMode>("file");
  const showValidation = shallowRef(false);
  const checkingServerId = shallowRef<string | null>(null);
  const installingAgentServerId = shallowRef<string | null>(null);
  const connectionCheck = shallowRef<ConnectionCheckState | null>(null);
  const copiedGuideCommand = shallowRef<string | null>(null);
  const accessSetup = shallowRef<RemoteServerAccessSetup | null>(null);
  const loadingAccessServerId = shallowRef<string | null>(null);
  const form = reactive({
    name: "",
    host: "",
    port: 22,
    username: "ignitify",
    deployPath: "/srv/ignitify",
    privateKeyText: "",
    publicKeyText: "",
    knownHosts: "",
    isDefault: true,
  });
  const selectedServer = computed(
    () => servers.value.find((server) => server.id === selectedServerId.value) ?? null,
  );
  const selectedConnectionCheck = computed(() =>
    connectionCheck.value?.serverId === selectedServerId.value ? connectionCheck.value : null,
  );
  const installPublicKeyCommand = computed(() => {
    if (!accessSetup.value) return "";
    const publicKey = accessSetup.value.publicKey.replaceAll("'", "'\\\\''");
    return `mkdir -p ~/.ssh && chmod 700 ~/.ssh && printf '%s\\n' '${publicKey}' >> ~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys`;
  });
  const publicKeyProvided = computed(() =>
    publicKeyMode.value === "text" ? !!form.publicKeyText.trim() : !!publicKeyFile.value,
  );
  const formError = computed(() => {
    if (!form.name.trim()) return "Server name is required.";
    if (!form.host.trim() || /[\s/@:]/.test(form.host.trim())) {
      return "Enter a hostname or IP address without a port.";
    }
    if (
      !Number.isInteger(Number(form.port)) ||
      Number(form.port) < 1 ||
      Number(form.port) > 65535
    ) {
      return "SSH port must be between 1 and 65535.";
    }
    if (!/^[a-z_][a-z0-9_-]{0,31}$/.test(form.username.trim())) {
      return "Enter a valid Linux SSH username.";
    }
    if (!editingId.value) return "";
    if (!form.deployPath.trim().startsWith("/")) {
      return "Deployment path must start with /.";
    }
    if (
      selectedServer.value &&
      !selectedServer.value.public_key_configured &&
      !publicKeyProvided.value
    ) {
      return "An SSH public key is required for this server.";
    }
    return "";
  });

  function resetForm() {
    form.name = "";
    form.host = "";
    form.port = 22;
    form.username = "ignitify";
    form.deployPath = "/srv/ignitify";
    form.privateKeyText = "";
    form.publicKeyText = "";
    form.knownHosts = "";
    form.isDefault = servers.value.length === 0;
    editingId.value = null;
    privateKeyFile.value = null;
    privateKeyInputKey.value += 1;
    privateKeyMode.value = "file";
    publicKeyFile.value = null;
    publicKeyInputKey.value += 1;
    publicKeyMode.value = "file";
    showValidation.value = false;
  }

  function updateDialog(open: boolean) {
    dialogOpen.value = open;
    if (!open) resetForm();
  }

  function addServer() {
    resetForm();
    dialogOpen.value = true;
  }

  function editServer(server: RemoteServerSummary) {
    form.name = server.name;
    form.host = server.host;
    form.port = server.port;
    form.username = server.username;
    form.deployPath = server.deploy_path;
    form.privateKeyText = "";
    form.publicKeyText = "";
    form.knownHosts = "";
    form.isDefault = server.is_default;
    editingId.value = server.id;
    privateKeyFile.value = null;
    privateKeyInputKey.value += 1;
    privateKeyMode.value = "file";
    publicKeyFile.value = null;
    publicKeyInputKey.value += 1;
    publicKeyMode.value = "file";
    showValidation.value = false;
    dialogOpen.value = true;
  }

  function selectServer(serverId: string) {
    if (selectedServerId.value !== serverId) connectionCheck.value = null;
    selectedServerId.value = serverId;
  }

  function closeInspector() {
    selectedServerId.value = null;
    connectionCheck.value = null;
  }

  async function checkConnection(server: RemoteServerSummary) {
    checkingServerId.value = server.id;
    connectionCheck.value = null;
    try {
      const result = await apiCheckRemoteServer(server.id);
      if (result.success) {
        connectionCheck.value = {
          serverId: server.id,
          status: "success",
          latencyMs: result.data.latency_ms,
          message: "SSH connection verified",
        };
        toast.success("SSH connection verified", {
          description: `${server.name} responded in ${result.data.latency_ms} ms.`,
        });
        return;
      }
      connectionCheck.value = {
        serverId: server.id,
        status: "error",
        message: result.error ?? "SSH connection failed",
      };
      toast.error("SSH connection failed", { description: connectionCheck.value.message });
    } catch {
      connectionCheck.value = {
        serverId: server.id,
        status: "error",
        message: "SSH connection check failed",
      };
      toast.error("SSH connection check failed");
    } finally {
      checkingServerId.value = null;
    }
  }

  async function installAgent(server: RemoteServerSummary) {
    installingAgentServerId.value = server.id;
    requestError.value = "";
    const result = await apiInstallRemoteServerAgent(server.id);
    if (!result.success) {
      requestError.value = result.error ?? "Unable to install the monitoring agent.";
      toast.error("Could not install monitoring agent", { description: requestError.value });
    } else if (await loadServers()) {
      toast.success("Monitoring agent installation started", { description: server.name });
    }
    installingAgentServerId.value = null;
  }

  function agentStatusLabel(server: RemoteServerSummary) {
    if (!server.agent) return "Agent not installed";
    if (server.agent.status === "online") return "Agent online";
    if (server.agent.status === "pending") return "Agent provisioning";
    return "Agent offline";
  }

  function agentStatusClass(server: RemoteServerSummary) {
    if (server.agent?.status === "online") return "bg-metric-green";
    if (server.agent?.status === "pending") return "bg-metric-amber";
    return "bg-muted-foreground";
  }

  function updatePrivateKey(event: Event) {
    privateKeyFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
  }

  function updatePublicKey(event: Event) {
    publicKeyFile.value = (event.target as HTMLInputElement).files?.[0] ?? null;
  }

  async function copyGuideCommand(command: string) {
    if (!navigator.clipboard) return;
    try {
      await navigator.clipboard.writeText(command);
      copiedGuideCommand.value = command;
      window.setTimeout(() => {
        if (copiedGuideCommand.value === command) copiedGuideCommand.value = null;
      }, 1_600);
      toast.success("Command copied");
    } catch {
      copiedGuideCommand.value = null;
      toast.error("Could not copy command");
    }
  }

  async function showAccessSetup(server: RemoteServerSummary) {
    loadingAccessServerId.value = server.id;
    const result = await apiGetRemoteServerAccess(server.id);
    loadingAccessServerId.value = null;
    if (!result.success) {
      const message = result.error ?? t("remoteServerOnboarding.accessLoadError");
      toast.error(t("remoteServerOnboarding.accessLoadFailed"), { description: message });
      return;
    }
    accessSetup.value = { server, publicKey: result.data.public_key };
    accessDialogOpen.value = true;
  }

  async function loadServers(showSuccess = false): Promise<boolean> {
    loading.value = true;
    requestError.value = "";
    const result = await apiListRemoteServers();
    if (result.success) {
      servers.value = result.data;
      if (!result.data.some((server) => server.id === selectedServerId.value)) {
        selectedServerId.value = null;
      }
      loading.value = false;
      if (showSuccess) toast.success("Remote servers refreshed");
      return true;
    }
    requestError.value = result.error ?? "Unable to load remote servers.";
    toast.error("Remote servers unavailable", { description: requestError.value });
    loading.value = false;
    return false;
  }

  async function refreshAgentStatuses() {
    const result = await apiListRemoteServers();
    if (result.success) servers.value = result.data;
  }

  async function saveServer() {
    showValidation.value = true;
    if (formError.value) return;
    saving.value = true;
    requestError.value = "";
    const wasEditing = Boolean(editingId.value);
    try {
      if (!editingId.value) {
        const result = await apiCreateRemoteServer({
          name: form.name.trim(),
          host: form.host.trim(),
          port: Number(form.port),
          username: form.username.trim(),
        });
        if (!result.success) {
          requestError.value = result.error ?? t("remoteServerOnboarding.createError");
          toast.error(t("remoteServerOnboarding.createFailed"), {
            description: requestError.value,
          });
          return;
        }
        const { public_key: publicKey, ...server } = result.data;
        if (!(await loadServers())) return;
        selectedServerId.value = server.id;
        updateDialog(false);
        accessSetup.value = { server, publicKey };
        accessDialogOpen.value = true;
        toast.success(t("remoteServerOnboarding.created"), { description: server.name });
        return;
      }
      const privateKey =
        privateKeyMode.value === "text"
          ? form.privateKeyText.trim() || undefined
          : privateKeyFile.value
            ? await privateKeyFile.value.text()
            : undefined;
      const publicKey =
        publicKeyMode.value === "text"
          ? form.publicKeyText.trim() || undefined
          : publicKeyFile.value
            ? await publicKeyFile.value.text()
            : undefined;
      const input: RemoteServerInput = {
        name: form.name.trim(),
        host: form.host.trim(),
        port: Number(form.port),
        username: form.username.trim(),
        deploy_path: form.deployPath.trim(),
        private_key: privateKey,
        public_key: publicKey,
        known_hosts: form.knownHosts.trim() || undefined,
        is_default: form.isDefault,
      };
      const result = await apiUpdateRemoteServer(editingId.value, input);
      if (!result.success) {
        requestError.value = result.error ?? "Unable to save remote server.";
        toast.error("Could not save remote server", { description: requestError.value });
        return;
      }
      if (!(await loadServers())) return;
      updateDialog(false);
      toast.success(wasEditing ? "Remote server updated" : "Remote server added", {
        description: input.name,
      });
    } catch {
      requestError.value = "Unable to read the SSH private key file.";
      toast.error("Could not read SSH key", { description: requestError.value });
    } finally {
      saving.value = false;
    }
  }

  async function setDefault(server: RemoteServerSummary) {
    if (server.is_default) return;
    requestError.value = "";
    const result = await apiSetDefaultRemoteServer(server.id);
    if (!result.success) {
      requestError.value = result.error ?? "Unable to update the default destination.";
      toast.error("Could not set default destination", { description: requestError.value });
      return;
    }
    servers.value = servers.value.map((item) =>
      item.id === result.data.id ? result.data : { ...item, is_default: false },
    );
    toast.success("Default destination updated", { description: server.name });
  }

  function requestDelete(server: RemoteServerSummary) {
    serverPendingDeletion.value = server;
    deleteDialogOpen.value = true;
  }

  async function removeServer() {
    const server = serverPendingDeletion.value;
    if (!server) return;
    removing.value = true;
    requestError.value = "";
    const result = await apiDeleteRemoteServer(server.id);
    removing.value = false;
    if (!result.success) {
      requestError.value = result.error ?? "Unable to remove remote server.";
      toast.error("Could not remove remote server", { description: requestError.value });
      return;
    }
    servers.value = servers.value.filter((item) => item.id !== server.id);
    closeInspector();
    serverPendingDeletion.value = null;
    deleteDialogOpen.value = false;
    toast.success("Remote server removed", { description: server.name });
  }

  let statusRefreshTimer: number | undefined;
  onMounted(() => {
    void loadServers();
    statusRefreshTimer = window.setInterval(() => void refreshAgentStatuses(), 30_000);
  });
  onUnmounted(() => {
    if (statusRefreshTimer !== undefined) window.clearInterval(statusRefreshTimer);
  });

  return {
    accessDialogOpen,
    accessSetup,
    addServer,
    agentStatusClass,
    agentStatusLabel,
    checkConnection,
    checkingServerId,
    closeInspector,
    connectionCheck,
    copiedGuideCommand,
    copyGuideCommand,
    deleteDialogOpen,
    dialogOpen,
    editServer,
    editingId,
    form,
    formError,
    installAgent,
    installPublicKeyCommand,
    installingAgentServerId,
    linuxGuideCommands,
    loadServers,
    loading,
    loadingAccessServerId,
    privateKeyFile,
    privateKeyInputKey,
    privateKeyMode,
    publicKeyFile,
    publicKeyInputKey,
    publicKeyMode,
    removeServer,
    removing,
    requestDelete,
    requestError,
    saving,
    selectedConnectionCheck,
    selectedServer,
    selectedServerId,
    selectServer,
    serverPendingDeletion,
    servers,
    setDefault,
    showAccessSetup,
    showValidation,
    t,
    updateDialog,
    updatePrivateKey,
    updatePublicKey,
    saveServer,
  };
}
