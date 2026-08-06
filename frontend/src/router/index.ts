import { createRouter, createWebHistory } from "vue-router";
import { useAuthStore } from "@/stores/auth";

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
      meta: { requiresAuth: true },
    },
    {
      path: "/projects",
      name: "Projects",
      component: () => import("@/views/ProjectsView.vue"),
      meta: { requiresAuth: true },
    },
    {
      path: "/containers",
      name: "DockerContainers",
      component: () => import("@/views/DockerContainersView.vue"),
      meta: { requiresAuth: true },
    },
    {
      path: "/projects/:projectId",
      name: "ProjectDetail",
      component: () => import("@/views/ProjectDetailView.vue"),
      meta: { requiresAuth: true },
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
  if (to.path === "/login" && authStore.isAuthenticated) return "/dashboard";
});

export default router;
