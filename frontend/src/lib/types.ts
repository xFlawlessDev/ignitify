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
