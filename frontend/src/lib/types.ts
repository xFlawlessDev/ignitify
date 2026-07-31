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
