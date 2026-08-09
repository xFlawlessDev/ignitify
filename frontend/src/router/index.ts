import type { RouteLocationRaw } from "vue-router";
import { createRouter, createWebHistory } from "vue-router";
import { useAuthStore } from "@/stores/auth";

declare module "vue-router" {
  interface RouteMeta {
    breadcrumb?: string;
    breadcrumbParam?: string;
    breadcrumbParent?: { label: string; to: RouteLocationRaw };
    requiresAdmin?: boolean;
  }
}

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: "/login",
      name: "Login",
      component: () => import("@/views/LoginView.vue"),
      meta: { layout: "blank" },
    },
    {
      path: "/dashboard",
      name: "Dashboard",
      component: () => import("@/views/DashboardView.vue"),
      meta: { requiresAuth: true, breadcrumb: "navigation.overview" },
    },
    {
      path: "/monitoring",
      name: "Monitoring",
      component: () => import("@/views/MonitoringView.vue"),
      meta: { requiresAuth: true, breadcrumb: "navigation.monitoring" },
    },
    {
      path: "/projects",
      name: "Projects",
      component: () => import("@/views/ProjectsView.vue"),
      meta: { requiresAuth: true, breadcrumb: "navigation.projects" },
    },
    {
      path: "/providers",
      name: "Providers",
      component: () => import("@/views/ProvidersView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.providers" },
    },
    {
      path: "/containers",
      name: "DockerContainers",
      component: () => import("@/views/DockerContainersView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.docker" },
    },
    {
      path: "/terminal",
      name: "Terminal",
      component: () => import("@/views/TerminalView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.terminal" },
    },
    {
      path: "/settings",
      name: "Settings",
      component: () => import("@/views/SettingsView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.settings" },
    },
    {
      path: "/remote-builders",
      name: "RemoteBuilders",
      component: () => import("@/views/RemoteBuildersView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.remoteBuilders" },
    },
    {
      path: "/remote-servers",
      name: "RemoteServers",
      component: () => import("@/views/RemoteServersView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "navigation.remoteServers" },
    },
    {
      path: "/projects/:projectId",
      name: "ProjectDetail",
      component: () => import("@/views/ProjectDetailView.vue"),
      meta: {
        requiresAuth: true,
        breadcrumb: "navigation.projects",
        breadcrumbParam: "projectId",
        breadcrumbParent: { label: "navigation.projects", to: { name: "Projects" } },
      },
    },
    {
      path: "/projects/:projectId/services/:serviceId",
      name: "ServiceDetail",
      component: () => import("@/views/ServiceDetailView.vue"),
      meta: {
        requiresAuth: true,
        breadcrumb: "Service",
        breadcrumbParent: { label: "navigation.projects", to: { name: "Projects" } },
      },
    },
    { path: "/", redirect: "/dashboard" },
  ],
});

router.beforeEach(async (to) => {
  const authStore = useAuthStore();
  await authStore.init();

  if (to.meta.requiresAuth && !authStore.isAuthenticated) {
    return { path: "/login", query: { redirect: to.fullPath } };
  }
  if (to.meta.requiresAdmin && !authStore.isAdmin) return "/dashboard";
  if (to.path === "/login" && authStore.isAuthenticated) return "/dashboard";
});

export default router;
