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

export type ProviderKind = "git" | "gitea" | "gitlab" | "github";
export type ProviderAuthMode = "token" | "oauth" | "github_app";

export interface ProviderSummary {
  id: string;
  name: string;
  kind: ProviderKind;
  auth_mode: ProviderAuthMode;
  base_url: string;
  internal_url: string | null;
  redirect_uri: string | null;
  client_id: string | null;
  application_id: string | null;
  installation_id: string | null;
  group_names: string | null;
  username: string | null;
  token_configured: boolean;
  created_at: string;
  updated_at: string;
  last_verified_at: string | null;
}

export interface ProviderInput {
  name: string;
  kind: ProviderKind;
  auth_mode: ProviderAuthMode;
  base_url: string;
  internal_url?: string;
  redirect_uri?: string;
  client_id?: string;
  client_secret?: string;
  application_id?: string;
  installation_id?: string;
  private_key?: string;
  group_names?: string;
  username?: string;
  token?: string;
}

export interface GithubManifestInput {
  name: string;
  base_url?: string;
}

export interface GithubManifestStart {
  action_url: string;
  manifest: Record<string, unknown>;
}

export interface ProviderConnectionResult {
  repository_count: number | null;
  checked_at: string;
}

export interface ProjectEnvironmentVariable {
  key: string;
  is_secret: boolean;
  is_set: boolean;
  value?: string;
}

export interface ProjectEnvironmentVariableInput {
  key: string;
  value?: string;
  is_secret: boolean;
}

export interface ProjectEnvironmentResponse {
  role: ProjectMemberRole;
  variables: ProjectEnvironmentVariable[];
}

export interface ServiceVariable {
  key: string;
  value: string;
  is_secret: boolean;
}

export type ServiceSource = "template" | "compose" | "application";
export type ApplicationBuilder = "static" | "spa" | "dockerfile" | "railpack";

export interface ServiceSourceConfig {
  source: ServiceSource;
  template?: string;
  provider_id?: string;
  repository?: string;
  branch?: string;
  builder?: ApplicationBuilder;
  dockerfile_path?: string;
  build_command?: string;
  output_directory?: string;
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
  source_config?: ServiceSourceConfig;
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

export interface SystemMetrics {
  cpu_usage_percentage: number;
  cpu_cores: number;
  memory_used_bytes: number;
  memory_total_bytes: number;
  disk_used_bytes: number;
  disk_total_bytes: number;
  docker_disk_used_bytes: number | null;
  docker_disk_total_bytes: number | null;
  block_read_bytes_per_second: number;
  block_write_bytes_per_second: number;
  network_receive_bytes_per_second: number;
  network_transmit_bytes_per_second: number;
}

export interface RuntimePort {
  container_port: number;
  host_ip: string | null;
  host_port: number | null;
  protocol: string;
}

export interface RuntimeContainer {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  health: string | null;
  ports: RuntimePort[];
  restart_count: number;
  cpu_percentage: number | null;
  memory_usage_bytes: number | null;
  cpu_limit_nano_cpus: number | null;
  memory_limit_bytes: number | null;
  managed: boolean;
}

export interface RuntimeContainerInventory {
  containers: RuntimeContainer[] | null;
}

export interface RuntimeContainerConfig {
  command: string[];
  entrypoint: string[];
  user: string | null;
  working_dir: string | null;
  tty: boolean;
  environment_keys: string[];
  labels: Array<{ key: string; value: string }>;
  restart_policy: string | null;
  privileged: boolean;
}

export interface RuntimeContainerMount {
  kind: string;
  source: string | null;
  destination: string | null;
  read_only: boolean;
}

export interface RuntimeContainerNetwork {
  name: string;
  ip_address: string | null;
  gateway: string | null;
  mac_address: string | null;
}

export interface RuntimeContainerDetails {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  config: RuntimeContainerConfig;
  mounts: RuntimeContainerMount[];
  networks: RuntimeContainerNetwork[];
}

export interface RuntimeContainerLogs {
  logs: string;
}

export interface ActivitySummary {
  id: string;
  action: string;
  resource_type: string | null;
  resource_id: string | null;
  created_at: string;
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
  source_config?: ServiceSourceConfig;
}
