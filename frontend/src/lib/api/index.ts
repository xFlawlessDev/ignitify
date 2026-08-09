// Barrel - re-exports all API functions from domain modules.
// All existing imports like `import { apiLogin } from "@/lib/api"` continue to work.

export * from "./core";
export * from "./activity";
export * from "./auth";
export * from "./dashboard";
export * from "./projects";
export * from "./providers";
export * from "./services";
export * from "./deployments";
export * from "./domains";
export * from "./settings";
export * from "./backup-destinations";
export * from "./remote-builders";
export * from "./remote-servers";
export * from "./uptime-monitors";
