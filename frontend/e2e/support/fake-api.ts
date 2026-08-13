import type { Page, Route } from "@playwright/test";

type Role = "platform_operator" | "admin" | "user";

interface FakeApiOptions {
  bootstrapRequired?: boolean;
  role?: Role;
}

export interface FakeApiState {
  authenticated: boolean;
  bootstrapped: boolean;
  role: Role;
  projects: Array<Record<string, unknown>>;
  services: Array<Record<string, unknown>>;
  deployments: Array<Record<string, unknown>>;
  settings: Record<string, unknown>;
  backupResponse: Record<string, unknown> | null;
  unhandledRequests: string[];
}

const now = "2026-08-13T00:00:00.000Z";
const environment = { id: "environment-1", name: "production", is_default: true };

export async function installFakeApi(
  page: Page,
  options: FakeApiOptions = {},
): Promise<FakeApiState> {
  const bootstrapRequired = options.bootstrapRequired ?? false;
  const state: FakeApiState = {
    authenticated: false,
    bootstrapped: !bootstrapRequired,
    role: options.role ?? "platform_operator",
    projects: [],
    services: [],
    deployments: [],
    settings: {
      application: { public_origin: "http://127.0.0.1:6565", secure_cookies: false },
      control_plane_domain: "",
      application_domain_suffix: "",
      https_enabled: true,
      automatically_provision_ssl: false,
      acme_email: "",
      dns_record_type: "a",
      dns_record_target: "",
      fallback_page_heading: "Application unavailable",
      fallback_page_message: "Please try again later.",
      certificate_provider: "none",
      custom_certificate_id: null,
      concurrent_builds: 2,
      certificates: [],
      health: { database: "ready", runtime: "ready", worker: "ready", ingress: "ready" },
      updated_at: now,
    },
    backupResponse: null,
    unhandledRequests: [],
  };

  await page.route("**/api/v1/**", async (route) => handleRequest(route, state));
  return state;
}

function authenticatedUser(state: FakeApiState) {
  return {
    id: "user-1",
    username: "operator",
    role: state.role,
    tenant_id: null,
    api_key_id: null,
    scopes: [],
  };
}

function session(state: FakeApiState) {
  return {
    access_token: "fixture-access-token",
    token_type: "Bearer",
    expires_at: "2026-08-13T01:00:00.000Z",
    user: authenticatedUser(state),
  };
}

function json(route: Route, body: unknown, status = 200) {
  return route.fulfill({ status, contentType: "application/json", body: JSON.stringify(body) });
}

function requestBody(route: Route): Record<string, unknown> {
  return (route.request().postDataJSON() as Record<string, unknown>) ?? {};
}

function stringField(body: Record<string, unknown>, key: string, fallback: string): string {
  const value = body[key];
  return typeof value === "string" ? value : fallback;
}

function projectFor(state: FakeApiState, id: string) {
  return state.projects.find((project) => project.id === id) ?? null;
}

function deploymentFor(state: FakeApiState, id: string) {
  return state.deployments.find((deployment) => deployment.id === id) ?? null;
}

async function handleRequest(route: Route, state: FakeApiState) {
  const request = route.request();
  const url = new URL(request.url());
  const path = url.pathname.replace(/^\/api\/v1/, "");
  const method = request.method();

  if (method === "GET" && path === "/auth/bootstrap") {
    return json(route, { required: !state.bootstrapped, enabled: true });
  }
  if (method === "POST" && path === "/auth/bootstrap") {
    state.bootstrapped = true;
    state.authenticated = true;
    return json(route, session(state));
  }
  if (method === "POST" && path === "/auth/login") {
    state.authenticated = true;
    return json(route, session(state));
  }
  if (method === "POST" && path === "/auth/refresh") {
    return state.authenticated
      ? json(route, session(state))
      : json(route, { error: "Session unavailable" }, 401);
  }
  if (method === "POST" && path === "/auth/logout") {
    state.authenticated = false;
    return json(route, { message: "Signed out" });
  }
  if (method === "GET" && path === "/auth/me") return json(route, authenticatedUser(state));

  if (!state.authenticated) return json(route, { error: "Authentication required" }, 401);

  if (method === "GET" && path === "/dashboard") {
    return json(route, {
      projects: state.projects,
      services: state.services,
      deployments: state.deployments,
    });
  }
  if (method === "GET" && path === "/runtime/status") {
    return json(route, { database: "ready", runtime: "ready", worker: "ready", metrics: null });
  }
  if (method === "GET" && path === "/projects") return json(route, state.projects);
  if (method === "POST" && path === "/projects") {
    const body = requestBody(route);
    const project = {
      id: `project-${state.projects.length + 1}`,
      name: stringField(body, "name", "Untitled project"),
      owner_id: "user-1",
      role: "owner",
      created_at: now,
      updated_at: now,
      default_environment: environment,
    };
    state.projects.push(project);
    return json(route, project);
  }

  const projectMatch = path.match(/^\/projects\/([^/]+)(?:\/(.*))?$/);
  if (projectMatch) {
    const [, projectId, suffix = ""] = projectMatch;
    const project = projectFor(state, projectId);
    if (!project) return json(route, { error: "Project not found" }, 404);
    if (method === "GET" && !suffix) return json(route, project);
    if (method === "GET" && suffix === "environment") {
      return json(route, { role: "owner", variables: [] });
    }
    if (method === "GET" && suffix === "services") {
      return json(
        route,
        state.services.filter((service) => service.project_id === projectId),
      );
    }
    if (method === "POST" && suffix === "services") {
      const body = requestBody(route);
      const service = {
        id: `service-${state.services.length + 1}`,
        project_id: projectId,
        environment_id: environment.id,
        role: "owner",
        name: stringField(body, "name", "web"),
        kind: "image",
        image_reference: "nginx:1.27",
        internal_port: 80,
        healthcheck: null,
        desired_generation: 1,
        desired_state: "running",
        created_at: now,
        updated_at: now,
        variables: [],
        source_config: { source: "template", template: "starter", setup_required: false },
      };
      state.services.push(service);
      return json(route, service);
    }
    if (method === "GET" && suffix === "deployments") return json(route, state.deployments);
    if (method === "GET" && suffix === "activity") return json(route, []);
  }

  const serviceMatch = path.match(/^\/services\/([^/]+)(?:\/(.*))?$/);
  if (serviceMatch) {
    const [, serviceId, suffix = ""] = serviceMatch;
    const service = state.services.find((candidate) => candidate.id === serviceId);
    if (!service) return json(route, { error: "Service not found" }, 404);
    if (method === "GET" && !suffix) return json(route, service);
    if (method === "GET" && suffix === "domains") return json(route, []);
    if (method === "GET" && suffix === "deployments") {
      return json(
        route,
        state.deployments.filter((deployment) => deployment.service_id === serviceId),
      );
    }
    if (method === "POST" && suffix === "deployments") {
      const deployment = {
        id: `deployment-${state.deployments.length + 1}`,
        service_id: serviceId,
        generation: 1,
        status: "healthy",
        failure_reason: null,
        attempt_count: 1,
        retry_after: null,
        cancel_requested_at: null,
        created_at: now,
        started_at: now,
        finished_at: now,
      };
      state.deployments.unshift(deployment);
      return json(route, deployment);
    }
  }

  if (method === "GET" && path === "/providers") return json(route, []);

  const streamMatch = path.match(/^\/deployments\/([^/]+)\/(events|logs)$/);
  if (method === "GET" && streamMatch) {
    const deployment = deploymentFor(state, streamMatch[1]);
    if (!deployment) return json(route, { error: "Deployment not found" }, 404);
    const body =
      streamMatch[2] === "events"
        ? `id: 1\nevent: snapshot\ndata: ${JSON.stringify({ deployment })}\n\n`
        : `id: 1\nevent: log\ndata: ${JSON.stringify({ sequence: 1, deployment_id: deployment.id, stream: "stdout", line: "deployment fixture complete", created_at: now })}\n\n`;
    return route.fulfill({ status: 200, contentType: "text/event-stream", body });
  }

  if (method === "GET" && path === "/settings/infrastructure") return json(route, state.settings);
  if (method === "PATCH" && path === "/settings/infrastructure") {
    state.settings = { ...state.settings, ...requestBody(route), updated_at: now };
    return json(route, state.settings);
  }
  if (method === "GET" && path === "/settings/backup-destination/s3") return json(route, null);
  if (method === "GET" && path === "/settings/backup-destination/s3/runs") return json(route, []);
  if (method === "PUT" && path === "/settings/backup-destination/s3") {
    const body = requestBody(route);
    state.backupResponse = {
      endpoint: body.endpoint,
      region: body.region,
      bucket: body.bucket,
      prefix: body.prefix,
      server_side_encryption: body.server_side_encryption,
      enabled: body.enabled,
      schedule_interval_hours: body.schedule_interval_hours,
      created_at: now,
      updated_at: now,
    };
    return json(route, state.backupResponse);
  }

  state.unhandledRequests.push(`${method} ${path}`);
  return json(route, { error: `Unhandled fixture request: ${method} ${path}` }, 500);
}
