<script setup lang="ts">
import { Background } from "@vue-flow/background";
import { Controls } from "@vue-flow/controls";
import { Handle, Position, useVueFlow, VueFlow, type Edge, type Node } from "@vue-flow/core";
import "@vue-flow/controls/dist/style.css";
import "@vue-flow/core/dist/style.css";
import { Box, GitBranch, Layers3, Rocket, Server, X } from "@lucide/vue";
import { computed, shallowRef, watch } from "vue";
import { useRouter } from "vue-router";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import type {
  DashboardProjectSummary,
  DashboardServiceSummary,
  DeploymentState,
  DeploymentSummary,
  RuntimeStatus,
} from "@/lib/types";

type TopologyNodeKind = "host" | "project" | "service" | "deployment";
type TopologyTone = "neutral" | "healthy" | "live" | "failed";

interface TopologyNodeData {
  kind: TopologyNodeKind;
  label: string;
  detail: string;
  status: string;
  tone: TopologyTone;
  projectId?: string;
  serviceId?: string;
  generation?: number;
}

interface Props {
  deployments: DeploymentSummary[];
  loading?: boolean;
  projects: DashboardProjectSummary[];
  runtime: RuntimeStatus | null;
  services: DashboardServiceSummary[];
}

const props = withDefaults(defineProps<Props>(), { loading: false });
const router = useRouter();
const flow = useVueFlow();
const flowNodes = shallowRef<Node<TopologyNodeData>[]>([]);
const flowEdges = shallowRef<Edge[]>([]);
const selectedNodeId = shallowRef<string | null>(null);

const deploymentLabels: Record<DeploymentState, string> = {
  failed: "Failed",
  healthy: "Healthy",
  preparing: "Preparing",
  queued: "Queued",
  running: "Running",
  stopped: "Stopped",
  stopping: "Stopping",
  superseded: "Superseded",
};

const selectedNode = computed(
  () => flowNodes.value.find((node) => node.id === selectedNodeId.value) ?? null,
);
const topologySummary = computed(() => {
  const active = [...latestDeployments().values()].filter((deployment) =>
    isActive(deployment.status),
  ).length;
  return active
    ? `${active} deployment${active === 1 ? "" : "s"} moving`
    : `${props.services.length} service${props.services.length === 1 ? "" : "s"} mapped`;
});

function isActive(status: DeploymentState) {
  return ["queued", "preparing", "running", "stopping"].includes(status);
}

function toneForDeployment(deployment: DeploymentSummary | undefined): TopologyTone {
  if (!deployment) return "neutral";
  if (deployment.status === "failed") return "failed";
  if (isActive(deployment.status)) return "live";
  if (deployment.status === "healthy") return "healthy";
  return "neutral";
}

function highestTone(tones: TopologyTone[]): TopologyTone {
  if (tones.includes("failed")) return "failed";
  if (tones.includes("live")) return "live";
  if (tones.includes("healthy")) return "healthy";
  return "neutral";
}

function statusDot(tone: TopologyTone) {
  if (tone === "failed") return "failed";
  if (tone === "live") return "live";
  if (tone === "healthy") return "healthy";
  return undefined;
}

function statusClass(tone: TopologyTone) {
  if (tone === "failed") return "text-destructive";
  if (tone === "live") return "text-[var(--status-live)]";
  if (tone === "healthy") return "text-[var(--status-healthy)]";
  return "text-muted-foreground";
}

function nodeBorderClass(tone: TopologyTone) {
  if (tone === "failed") return "border-destructive/60 hover:border-destructive";
  if (tone === "live") return "border-[var(--status-live)] hover:border-[var(--status-live)]";
  if (tone === "healthy") {
    return "border-[var(--status-healthy)] hover:border-[var(--status-healthy)]";
  }
  return "border-border hover:border-ring";
}

function edgeStyle(tone: TopologyTone) {
  if (tone === "failed") return { stroke: "var(--destructive)", strokeWidth: 1.5 };
  if (tone === "live") return { stroke: "var(--status-live)", strokeWidth: 1.5 };
  if (tone === "healthy") return { stroke: "var(--status-healthy)", strokeWidth: 1.5 };
  return { stroke: "var(--border)", strokeWidth: 1.25 };
}

function latestDeployments() {
  const ordered = [...props.deployments].sort((left, right) =>
    right.created_at.localeCompare(left.created_at),
  );
  const latest = new Map<string, DeploymentSummary>();
  for (const deployment of ordered) {
    if (!latest.has(deployment.service_id)) latest.set(deployment.service_id, deployment);
  }
  return latest;
}

function updateTopology() {
  const servicesByProject = new Map<string, DashboardServiceSummary[]>();
  for (const service of props.services) {
    const services = servicesByProject.get(service.project_id) ?? [];
    services.push(service);
    servicesByProject.set(service.project_id, services);
  }
  const latestByService = latestDeployments();
  const hostReady =
    props.runtime &&
    [props.runtime.database, props.runtime.runtime, props.runtime.worker].every(
      (status) => status === "ready",
    );
  const hostTone: TopologyTone = props.runtime ? (hostReady ? "healthy" : "failed") : "neutral";
  const nodes: Node<TopologyNodeData>[] = [];
  const edges: Edge[] = [];
  const rowHeight = 124;
  let cursorY = 48;

  for (const project of props.projects) {
    const projectServices = servicesByProject.get(project.id) ?? [];
    const serviceTones = projectServices.map((service) =>
      toneForDeployment(latestByService.get(service.id)),
    );
    const projectTone = highestTone(serviceTones);
    const projectHeight = Math.max(rowHeight, projectServices.length * rowHeight);
    const projectY = cursorY + Math.max(0, (projectHeight - 80) / 2);
    const projectId = `project:${project.id}`;

    nodes.push({
      id: projectId,
      type: "project",
      label: project.name,
      position: { x: 304, y: projectY },
      data: {
        kind: "project",
        label: project.name,
        detail: `${projectServices.length} configured service${projectServices.length === 1 ? "" : "s"}`,
        projectId: project.id,
        status:
          projectTone === "failed"
            ? "Needs attention"
            : projectTone === "live"
              ? "Deployment activity"
              : projectTone === "healthy"
                ? "Healthy"
                : "No deployment data",
        tone: projectTone,
      },
      draggable: false,
      selectable: false,
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
    });

    edges.push({
      id: `host-${projectId}`,
      source: "host",
      target: projectId,
      type: "smoothstep",
      selectable: false,
      focusable: false,
      style: edgeStyle(projectTone),
    });

    for (const [serviceIndex, service] of projectServices.entries()) {
      const deployment = latestByService.get(service.id);
      const serviceTone = toneForDeployment(deployment);
      const serviceId = `service:${service.id}`;
      const deploymentId = deployment ? `deployment:${deployment.id}` : null;
      const serviceY = cursorY + serviceIndex * rowHeight;

      nodes.push({
        id: serviceId,
        type: "service",
        label: service.name,
        position: { x: 570, y: serviceY },
        data: {
          kind: "service",
          label: service.name,
          detail: `${service.kind} service / target ${service.desired_state}`,
          projectId: project.id,
          serviceId: service.id,
          status: deployment ? deploymentLabels[deployment.status] : "Awaiting deployment",
          tone: serviceTone,
        },
        draggable: false,
        selectable: false,
        sourcePosition: Position.Right,
        targetPosition: Position.Left,
      });
      edges.push({
        id: `${projectId}-${serviceId}`,
        source: projectId,
        target: serviceId,
        type: "smoothstep",
        selectable: false,
        focusable: false,
        style: edgeStyle(serviceTone),
      });

      if (deployment && deploymentId) {
        nodes.push({
          id: deploymentId,
          type: "deployment",
          label: `Deployment g${deployment.generation}`,
          position: { x: 834, y: serviceY },
          data: {
            kind: "deployment",
            label: `Deployment g${deployment.generation}`,
            detail: deployment.failure_reason ?? `Latest rollout for ${service.name}`,
            generation: deployment.generation,
            projectId: project.id,
            serviceId: service.id,
            status: deploymentLabels[deployment.status],
            tone: serviceTone,
          },
          draggable: false,
          selectable: false,
          targetPosition: Position.Left,
        });
        edges.push({
          id: `${serviceId}-${deploymentId}`,
          source: serviceId,
          target: deploymentId,
          type: "smoothstep",
          animated: isActive(deployment.status),
          selectable: false,
          focusable: false,
          style: edgeStyle(serviceTone),
        });
      }
    }

    cursorY += projectHeight + 36;
  }

  nodes.unshift({
    id: "host",
    type: "host",
    label: "This Ignitify host",
    position: { x: 44, y: Math.max(92, cursorY / 2 - 42) },
    data: {
      kind: "host",
      label: "This Ignitify host",
      detail: props.runtime ? "Control plane runtime" : "Runtime status unavailable",
      status: hostReady ? "Runtime ready" : props.runtime ? "Runtime degraded" : "Runtime unknown",
      tone: hostTone,
    },
    draggable: false,
    selectable: false,
    sourcePosition: Position.Right,
  });

  flowNodes.value = nodes;
  flowEdges.value = edges;
  if (!nodes.some((node) => node.id === selectedNodeId.value)) selectedNodeId.value = null;
}

function selectNode(nodeId: string) {
  selectedNodeId.value = nodeId;
}

function closeInspector() {
  selectedNodeId.value = null;
}

function openSelectedNode() {
  const node = selectedNode.value;
  if (!node?.data.projectId) return;
  if (node.data.serviceId) {
    void router.push({
      name: "ServiceDetail",
      params: { projectId: node.data.projectId, serviceId: node.data.serviceId },
    });
    return;
  }
  void router.push({ name: "ProjectDetail", params: { projectId: node.data.projectId } });
}

function actionLabel(node: Node<TopologyNodeData>) {
  return node.data.serviceId ? "Open service" : "Open project";
}

watch(() => [props.deployments, props.projects, props.runtime, props.services], updateTopology, {
  immediate: true,
});

flow.onPaneClick(closeInspector);
</script>

<template>
  <section class="mt-6 app-surface overflow-hidden" aria-labelledby="deployment-topology-title">
    <header class="app-panel-header flex flex-wrap items-start justify-between gap-4 px-5 py-4">
      <div>
        <p class="ui-label">Deployment map</p>
        <h2 id="deployment-topology-title" class="mt-2 text-base font-medium">Runtime topology</h2>
      </div>
      <div
        class="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-[10px] text-muted-foreground"
      >
        <span class="flex items-center gap-1.5">
          <span class="status-dot" data-status="healthy" aria-hidden="true" />Healthy
        </span>
        <span class="flex items-center gap-1.5">
          <span class="status-dot" data-status="live" aria-hidden="true" />Deploying
        </span>
        <span class="flex items-center gap-1.5">
          <span class="status-dot" data-status="failed" aria-hidden="true" />Attention
        </span>
        <span class="text-border" aria-hidden="true">/</span>
        <span>{{ topologySummary }}</span>
      </div>
    </header>

    <div class="relative h-[620px] max-[640px]:h-[520px]">
      <div v-if="loading" class="grid size-full grid-cols-4 gap-6 p-8" role="status">
        <div v-for="index in 4" :key="index" class="grid content-center gap-3">
          <Skeleton class="h-3 w-20" />
          <Skeleton class="h-24 w-full" />
        </div>
      </div>
      <VueFlow
        v-else
        class="size-full bg-muted/35 [&_.vue-flow__controls-button:last-child]:border-b-0 [&_.vue-flow__controls-button:hover]:bg-muted [&_.vue-flow__controls-button]:size-[18px] [&_.vue-flow__controls-button]:border-b [&_.vue-flow__controls-button]:border-border [&_.vue-flow__controls-button]:bg-card [&_.vue-flow__controls-button]:text-foreground [&_.vue-flow__controls]:overflow-hidden [&_.vue-flow__controls]:rounded-[3px] [&_.vue-flow__controls]:border [&_.vue-flow__controls]:border-border [&_.vue-flow__controls]:shadow-none"
        v-model:nodes="flowNodes"
        v-model:edges="flowEdges"
        :min-zoom="0.4"
        :max-zoom="1.4"
        :nodes-draggable="false"
        :nodes-connectable="false"
        :elements-selectable="false"
        :zoom-on-double-click="false"
        :fit-view-on-init="true"
        :default-viewport="{ x: 0, y: 0, zoom: 1 }"
      >
        <Background :gap="20" :size="1" color="var(--border)" />
        <Controls position="bottom-right" :show-interactive="false" />

        <template #node-host="{ data, id }">
          <button
            class="nodrag nopan nowheel grid min-h-[104px] w-[204px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border bg-card p-4 text-left text-foreground transition-[border-color] duration-150 motion-reduce:transition-none"
            :class="[
              nodeBorderClass(data.tone),
              id === selectedNodeId ? 'ring-2 ring-ring/35' : '',
            ]"
            type="button"
            :aria-pressed="id === selectedNodeId"
            @pointerdown.stop
            @mousedown.stop
            @click.stop="selectNode(id)"
          >
            <Handle
              type="source"
              :position="Position.Right"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <span
              class="grid size-8 place-items-center rounded-[4px] border border-border bg-muted"
            >
              <Server class="size-4 text-muted-foreground" :stroke-width="1.5" />
            </span>
            <div class="min-w-0">
              <p class="ui-label">Host</p>
              <p class="mt-1 truncate text-xs font-medium">{{ data.label }}</p>
              <p
                class="mt-2 flex items-center gap-1.5 font-mono text-[10px]"
                :class="statusClass(data.tone)"
              >
                <span class="status-dot" :data-status="statusDot(data.tone)" aria-hidden="true" />
                {{ data.status }}
              </p>
            </div>
          </button>
        </template>

        <template #node-project="{ data, id }">
          <button
            class="nodrag nopan nowheel grid min-h-[104px] w-[204px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border bg-card p-4 text-left text-foreground transition-[border-color] duration-150 motion-reduce:transition-none"
            :class="[
              nodeBorderClass(data.tone),
              id === selectedNodeId ? 'ring-2 ring-ring/35' : '',
            ]"
            type="button"
            :aria-pressed="id === selectedNodeId"
            @pointerdown.stop
            @mousedown.stop
            @click.stop="selectNode(id)"
          >
            <Handle
              type="target"
              :position="Position.Left"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <Handle
              type="source"
              :position="Position.Right"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <span
              class="grid size-8 place-items-center rounded-[4px] border border-border bg-muted"
            >
              <Box class="size-4 text-muted-foreground" :stroke-width="1.5" />
            </span>
            <div class="min-w-0">
              <p class="ui-label">Project</p>
              <p class="mt-1 truncate text-xs font-medium">{{ data.label }}</p>
              <p class="mt-1 truncate text-[10px] text-muted-foreground">{{ data.detail }}</p>
              <p
                class="mt-1 flex items-center gap-1.5 font-mono text-[10px]"
                :class="statusClass(data.tone)"
              >
                <span class="status-dot" :data-status="statusDot(data.tone)" aria-hidden="true" />
                {{ data.status }}
              </p>
            </div>
          </button>
        </template>

        <template #node-service="{ data, id }">
          <button
            class="nodrag nopan nowheel grid min-h-[104px] w-[204px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border bg-card p-4 text-left text-foreground transition-[border-color] duration-150 motion-reduce:transition-none"
            :class="[
              nodeBorderClass(data.tone),
              id === selectedNodeId ? 'ring-2 ring-ring/35' : '',
            ]"
            type="button"
            :aria-pressed="id === selectedNodeId"
            @pointerdown.stop
            @mousedown.stop
            @click.stop="selectNode(id)"
          >
            <Handle
              type="target"
              :position="Position.Left"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <Handle
              type="source"
              :position="Position.Right"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <span
              class="grid size-8 place-items-center rounded-[4px] border border-border bg-muted"
            >
              <Layers3 class="size-4 text-muted-foreground" :stroke-width="1.5" />
            </span>
            <div class="min-w-0">
              <p class="ui-label">Service</p>
              <p class="mt-1 truncate text-xs font-medium">{{ data.label }}</p>
              <p
                class="mt-2 flex items-center gap-1.5 font-mono text-[10px]"
                :class="statusClass(data.tone)"
              >
                <span
                  class="status-dot"
                  :class="data.tone === 'live' ? 'animate-pulse motion-reduce:animate-none' : ''"
                  :data-status="statusDot(data.tone)"
                  aria-hidden="true"
                />
                {{ data.status }}
              </p>
            </div>
          </button>
        </template>

        <template #node-deployment="{ data, id }">
          <button
            class="nodrag nopan nowheel grid min-h-[104px] w-[204px] grid-cols-[32px_minmax(0,1fr)] gap-3 rounded-[8px] border bg-card p-4 text-left text-foreground transition-[border-color] duration-150 motion-reduce:transition-none"
            :class="[
              nodeBorderClass(data.tone),
              id === selectedNodeId ? 'ring-2 ring-ring/35' : '',
            ]"
            type="button"
            :aria-pressed="id === selectedNodeId"
            @pointerdown.stop
            @mousedown.stop
            @click.stop="selectNode(id)"
          >
            <Handle
              type="target"
              :position="Position.Left"
              class="size-2 min-h-2 min-w-2 rounded-full border border-card bg-muted-foreground"
            />
            <span
              class="grid size-8 place-items-center rounded-[4px] border border-border bg-muted"
            >
              <Rocket class="size-4 text-muted-foreground" :stroke-width="1.5" />
            </span>
            <div class="min-w-0">
              <p class="ui-label">Deployment</p>
              <p class="mt-1 truncate text-xs font-medium">{{ data.label }}</p>
              <p
                class="mt-2 flex items-center gap-1.5 font-mono text-[10px]"
                :class="statusClass(data.tone)"
              >
                <span
                  class="status-dot"
                  :class="data.tone === 'live' ? 'animate-pulse motion-reduce:animate-none' : ''"
                  :data-status="statusDot(data.tone)"
                  aria-hidden="true"
                />
                {{ data.status }}
              </p>
            </div>
          </button>
        </template>
      </VueFlow>

      <aside
        v-if="selectedNode"
        class="absolute inset-x-3 top-3 z-10 max-h-[calc(100%_-_1.5rem)] overflow-y-auto rounded-[8px] border border-border bg-card sm:left-auto sm:right-4 sm:w-[288px]"
        aria-labelledby="topology-inspector-title"
      >
        <header class="flex items-start justify-between gap-3 border-b border-border px-4 py-3">
          <div class="min-w-0">
            <p class="ui-label">Inspector</p>
            <h3 id="topology-inspector-title" class="mt-1.5 truncate text-base font-medium">
              {{ selectedNode.data.label }}
            </h3>
          </div>
          <Button
            variant="ghost"
            size="icon-sm"
            class="shrink-0"
            type="button"
            aria-label="Close topology inspector"
            title="Close topology inspector"
            @click="closeInspector"
          >
            <X class="size-4" :stroke-width="1.5" />
          </Button>
        </header>
        <div class="space-y-4 px-4 py-4">
          <dl class="grid gap-3 text-xs">
            <div class="flex items-start justify-between gap-4">
              <dt class="text-muted-foreground">Node</dt>
              <dd class="font-mono text-[11px] capitalize">{{ selectedNode.data.kind }}</dd>
            </div>
            <div class="flex items-start justify-between gap-4">
              <dt class="text-muted-foreground">Status</dt>
              <dd
                class="flex items-center gap-1.5 text-right"
                :class="statusClass(selectedNode.data.tone)"
              >
                <span
                  class="status-dot"
                  :data-status="statusDot(selectedNode.data.tone)"
                  aria-hidden="true"
                />
                {{ selectedNode.data.status }}
              </dd>
            </div>
            <div v-if="selectedNode.data.generation" class="flex items-start justify-between gap-4">
              <dt class="text-muted-foreground">Generation</dt>
              <dd class="font-mono text-[11px]">g{{ selectedNode.data.generation }}</dd>
            </div>
          </dl>
          <p class="border-t border-border pt-4 text-xs leading-5 text-muted-foreground">
            {{ selectedNode.data.detail }}
          </p>
          <dl
            v-if="selectedNode.data.kind === 'host' && runtime?.metrics"
            class="grid grid-cols-2 gap-x-5 gap-y-3 border-t border-border pt-4 text-xs"
          >
            <div>
              <dt class="text-muted-foreground">Containers</dt>
              <dd class="mt-1 font-mono text-foreground">
                {{ runtime.metrics.containers_running }}/{{ runtime.metrics.containers }}
              </dd>
            </div>
            <div>
              <dt class="text-muted-foreground">Images</dt>
              <dd class="mt-1 font-mono text-foreground">{{ runtime.metrics.images }}</dd>
            </div>
          </dl>
          <Button
            v-if="selectedNode.data.projectId"
            class="w-full"
            size="sm"
            type="button"
            @click="openSelectedNode"
          >
            <GitBranch class="size-4" :stroke-width="1.5" />
            {{ actionLabel(selectedNode) }}
          </Button>
        </div>
      </aside>
    </div>
  </section>
</template>

<style scoped>
@media (prefers-reduced-motion: reduce) {
  :deep(.vue-flow__edge.animated path) {
    animation: none;
  }
}
</style>
