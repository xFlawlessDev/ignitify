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
      meta: { requiresAuth: true, breadcrumb: "Overview" },
    },
    {
      path: "/monitoring",
      name: "Monitoring",
      component: () => import("@/views/MonitoringView.vue"),
      meta: { requiresAuth: true, breadcrumb: "Monitoring" },
    },
    {
      path: "/projects",
      name: "Projects",
      component: () => import("@/views/ProjectsView.vue"),
      meta: { requiresAuth: true, breadcrumb: "Projects" },
    },
    {
      path: "/containers",
      name: "DockerContainers",
      component: () => import("@/views/DockerContainersView.vue"),
      meta: { requiresAuth: true, breadcrumb: "Docker" },
    },
    {
      path: "/terminal",
      name: "Terminal",
      component: () => import("@/views/TerminalView.vue"),
      meta: { requiresAuth: true, requiresAdmin: true, breadcrumb: "Terminal" },
    },
    {
      path: "/projects/:projectId",
      name: "ProjectDetail",
      component: () => import("@/views/ProjectDetailView.vue"),
      meta: {
        requiresAuth: true,
        breadcrumb: "Project",
        breadcrumbParam: "projectId",
        breadcrumbParent: { label: "Projects", to: { name: "Projects" } },
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
