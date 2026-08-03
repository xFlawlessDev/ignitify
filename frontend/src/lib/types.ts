export type UserRole = "admin" | "user";

export interface AuthenticatedUser {
  id: string;
  username: string;
  role: UserRole;
  tenant_id: string | null;
  api_key_id: string | null;
  scopes: string[];
}

export interface AuthSession {
  access_token: string;
  token_type: "Bearer";
  expires_at: string;
  user: AuthenticatedUser;
}

export interface MessageResponse {
  message: string;
}

export type ProjectMemberRole = "owner" | "editor" | "viewer";

export interface EnvironmentSummary {
  id: string;
  name: string;
  is_default: boolean;
}

export interface ProjectSummary {
  id: string;
  name: string;
  owner_id: string;
  role: ProjectMemberRole;
  created_at: string;
  updated_at: string;
  default_environment: EnvironmentSummary;
}

export interface ProjectInput {
  name: string;
}

export interface ServiceVariable {
  key: string;
  value: string;
  is_secret: boolean;
}

export interface ServiceVariableSummary {
  key: string;
  is_secret: boolean;
  is_set: boolean;
  value?: string;
}

export interface ServiceInput {
  name: string;
  kind: "image" | "compose";
  image_reference?: string;
  compose_yaml?: string;
  exposed_service?: string;
  internal_port: number | null;
  healthcheck: string[] | null;
  variables: ServiceVariable[];
}

export type DeploymentState =
  | "queued"
  | "preparing"
  | "running"
  | "healthy"
  | "failed"
  | "stopping"
  | "stopped"
  | "superseded";

export interface DomainSummary {
  id: string;
  service_id: string;
  hostname: string;
  status: "pending" | "active" | "failed";
  last_error: string | null;
  created_at: string;
  updated_at: string;
}

export interface DeploymentEvent {
  sequence: number;
  deployment_id: string;
  kind: string;
  created_at: string;
  payload: Record<string, unknown>;
}

export interface DeploymentLog {
  sequence: number;
  deployment_id: string;
  stream: "stdout" | "stderr" | "system";
  line: string;
  created_at: string;
}

export interface DeploymentSummary {
  id: string;
  service_id: string;
  generation: number;
  status: DeploymentState;
  failure_reason: string | null;
  created_at: string;
  started_at: string | null;
  finished_at: string | null;
}

export interface DashboardProjectSummary {
  id: string;
  name: string;
}

export interface DashboardServiceSummary {
  id: string;
  project_id: string;
  name: string;
  kind: "image" | "compose";
  desired_generation: number;
  desired_state: "running" | "stopped";
}

export interface DashboardSummary {
  projects: DashboardProjectSummary[];
  services: DashboardServiceSummary[];
  deployments: DeploymentSummary[];
}

export type RuntimeComponentStatus = "ready" | "unavailable";

export interface RuntimeMetrics {
  containers: number;
  containers_running: number;
  images: number;
  cpus: number;
  memory_bytes: number;
}

export interface RuntimeStatus {
  database: RuntimeComponentStatus;
  runtime: RuntimeComponentStatus;
  worker: RuntimeComponentStatus;
  metrics: RuntimeMetrics | null;
}

export interface ActivitySummary {
  id: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  created_at: string;
}

export interface RegistrySummary {
  id: string;
  name: string;
  endpoint: string;
  username: string | null;
  credential_configured: boolean;
  created_at: string;
  updated_at: string;
}

export interface RegistryInput {
  name: string;
  endpoint: string;
  username?: string;
  credential?: string;
}

export interface WebhookSummary {
  id: string;
  project_id: string;
  name: string;
  url: string;
  secret_configured: boolean;
  is_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface WebhookInput {
  name: string;
  url: string;
  secret?: string;
  is_enabled?: boolean;
}

export interface TerminalCapability {
  available: boolean;
  reason: string;
}

export interface ServiceSummary {
  id: string;
  project_id: string;
  environment_id: string;
  role: ProjectMemberRole;
  name: string;
  kind: "image" | "compose";
  image_reference?: string;
  compose_yaml?: string;
  exposed_service?: string;
  internal_port: number | null;
  healthcheck: string[] | null;
  desired_generation: number;
  desired_state: "running" | "stopped";
  created_at: string;
  updated_at: string;
  variables: ServiceVariableSummary[];
}
