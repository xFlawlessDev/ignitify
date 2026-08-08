import { describe, expect, it } from "vitest";
import { templateRuntimeDefaults, type TemplateApplication } from "./template-catalog";

describe("templateRuntimeDefaults", () => {
  it("maps template variables, environment entries, secrets, and domain runtime details", () => {
    const application: TemplateApplication = {
      template: {
        id: "demo",
        name: "Demo",
        version: "1.0.0",
        description: "Demo template",
        logo: "",
        links: {},
        tags: [],
      },
      composeYaml: "services:\n  fallback:\n    image: example/demo:1\n",
      templateToml: `
[variables]
public_host = "app.example.test"
api_secret = "\${password:48}"

[config]
env = [
  "PUBLIC_HOST=\${public_host}",
  "API_SECRET=\${api_secret}",
  "LOG_LEVEL=info",
]

[[config.domains]]
serviceName = "web"
port = 8080
host = "\${public_host}"
`,
    };

    const defaults = templateRuntimeDefaults(application);

    expect(defaults.exposedService).toBe("web");
    expect(defaults.internalPort).toBe("8080");
    expect(defaults.variables.find((variable) => variable.key === "PUBLIC_HOST")).toEqual({
      key: "PUBLIC_HOST",
      value: "app.example.test",
      is_secret: false,
    });
    expect(defaults.variables.find((variable) => variable.key === "LOG_LEVEL")).toEqual({
      key: "LOG_LEVEL",
      value: "info",
      is_secret: false,
    });
    const apiSecret = defaults.variables.find((variable) => variable.key === "API_SECRET");
    expect(apiSecret?.is_secret).toBe(true);
    expect(apiSecret?.value.length).toBe(48);
    expect(/^[A-Za-z0-9_-]+$/.test(apiSecret?.value ?? "")).toBe(true);
  });

  it("leaves template domain inputs blank for the operator to supply", () => {
    const defaults = templateRuntimeDefaults({
      template: {
        id: "domain-input",
        name: "Domain input",
        version: "1.0.0",
        description: "",
        logo: "",
        links: {},
        tags: [],
      },
      composeYaml: "services:\n  web:\n    image: example/web:1\n",
      templateToml: `
[variables]
main_domain = "\${domain}"

[config]
env = ["ROUTER_HOST=\${main_domain}"]

[[config.domains]]
serviceName = "web"
port = "3000"
host = "\${main_domain}"
`,
    });

    expect(defaults.variables).toEqual([{ key: "ROUTER_HOST", value: "", is_secret: false }]);
  });
});
