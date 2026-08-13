#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const [outputPath] = process.argv.slice(2);

if (!outputPath || process.argv.length !== 3) {
  console.error("Usage: scripts/generate-frontend-sbom.mjs OUTPUT_FILE");
  process.exit(1);
}

const frontendDirectory = resolve(import.meta.dirname, "../frontend");
const output = resolve(outputPath);
const dependencyTree = readDependencyTree();
const rootRef = packageUrl(dependencyTree.name, dependencyTree.version);
const components = new Map();
const dependencies = new Map();

visitDependencies(rootRef, dependencyTree.dependencies);

const bom = {
  bomFormat: "CycloneDX",
  specVersion: "1.5",
  version: 1,
  metadata: {
    component: {
      type: "application",
      "bom-ref": rootRef,
      name: dependencyTree.name,
      version: dependencyTree.version,
      purl: rootRef,
    },
  },
  components: [...components.values()].sort((left, right) =>
    left["bom-ref"].localeCompare(right["bom-ref"]),
  ),
  dependencies: [...dependencies.entries()]
    .map(([ref, dependsOn]) => ({
      ref,
      dependsOn: [...dependsOn].sort(),
    }))
    .sort((left, right) => left.ref.localeCompare(right.ref)),
};

mkdirSync(dirname(output), { recursive: true });
writeFileSync(output, `${JSON.stringify(bom, null, 2)}\n`, { mode: 0o600 });
console.log(`[ignitify-sbom] created ${output}`);

function readDependencyTree() {
  const command = process.platform === "win32" ? "pnpm.cmd" : "pnpm";
  const commandArguments = ["list", "--prod", "--json", "--depth", "Infinity"];
  const options = {
    cwd: frontendDirectory,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
    stdio: ["ignore", "pipe", "inherit"],
  };
  const output = process.platform === "win32"
    ? execFileSync("cmd.exe", ["/d", "/s", "/c", command, ...commandArguments], options)
    : execFileSync(command, commandArguments, options);
  const trees = JSON.parse(output);

  if (!Array.isArray(trees) || trees.length !== 1) {
    throw new Error("pnpm did not return exactly one frontend dependency tree");
  }

  const tree = trees[0];
  if (!isRecord(tree) || !isPackageIdentity(tree.name, tree.version)) {
    throw new Error("pnpm returned an invalid frontend package identity");
  }
  return tree;
}

function visitDependencies(parentRef, entries) {
  const parentDependencies = dependencies.get(parentRef) ?? new Set();
  dependencies.set(parentRef, parentDependencies);

  if (!isRecord(entries)) {
    return;
  }

  for (const [name, entry] of Object.entries(entries)) {
    if (!isRecord(entry) || !isPackageIdentity(name, entry.version)) {
      throw new Error(`pnpm returned an invalid dependency entry for ${name}`);
    }

    const ref = packageUrl(name, entry.version);
    parentDependencies.add(ref);
    if (components.has(ref)) {
      continue;
    }

    components.set(ref, {
      type: "library",
      "bom-ref": ref,
      name,
      version: entry.version,
      purl: ref,
    });
    visitDependencies(ref, entry.dependencies);
  }
}

function isPackageIdentity(name, version) {
  return typeof name === "string" && name.length > 0 && typeof version === "string" && version.length > 0;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function packageUrl(name, version) {
  const packageName = name.split("/").map(encodeURIComponent).join("/");
  return `pkg:npm/${packageName}@${encodeURIComponent(version)}`;
}
