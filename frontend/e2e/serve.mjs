import { createServer } from "node:http";
import { readFile, stat } from "node:fs/promises";
import { dirname, extname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const distDirectory = resolve(__dirname, "..", "dist");
const args = process.argv.slice(2);
const host = valueFor("--host") ?? "127.0.0.1";
const port = Number(valueFor("--port") ?? "6565");
const contentTypes = {
  ".css": "text/css; charset=utf-8",
  ".html": "text/html; charset=utf-8",
  ".ico": "image/x-icon",
  ".js": "text/javascript; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".svg": "image/svg+xml",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};

function valueFor(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function isWithinDist(path) {
  const pathFromDist = relative(distDirectory, path);
  return (
    pathFromDist === "" ||
    (!pathFromDist.startsWith(`..${sep}`) && pathFromDist !== ".." && !isAbsolute(pathFromDist))
  );
}

async function fileExists(path) {
  try {
    return (await stat(path)).isFile();
  } catch {
    return false;
  }
}

if (!(await fileExists(resolve(distDirectory, "index.html")))) {
  throw new Error(`Frontend bundle is unavailable at ${distDirectory}`);
}

const server = createServer(async (request, response) => {
  if (request.method !== "GET" && request.method !== "HEAD") {
    response.writeHead(405, { Allow: "GET, HEAD" });
    response.end();
    return;
  }

  let pathname;
  try {
    pathname = decodeURIComponent(new URL(request.url ?? "/", "http://localhost").pathname);
  } catch {
    response.writeHead(400);
    response.end();
    return;
  }

  const assetPath = resolve(distDirectory, `.${pathname}`);
  if (!isWithinDist(assetPath)) {
    response.writeHead(403);
    response.end();
    return;
  }

  const requestedAsset = await fileExists(assetPath);
  if (!requestedAsset && extname(pathname)) {
    response.writeHead(404);
    response.end();
    return;
  }

  const filePath = requestedAsset ? assetPath : resolve(distDirectory, "index.html");
  const contentType = contentTypes[extname(filePath)] ?? "application/octet-stream";
  response.writeHead(200, { "Content-Type": contentType });
  if (request.method === "HEAD") {
    response.end();
    return;
  }
  response.end(await readFile(filePath));
});

server.listen(port, host, () => {
  process.stdout.write(`Serving frontend bundle at http://${host}:${port}\n`);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
