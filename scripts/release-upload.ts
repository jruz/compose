import { log } from "./lib/log.ts";
import { Glob } from "bun";

const parseId = (data: unknown): number => {
  if (typeof data !== "object" || data === null || !("id" in data))
    throw new Error("missing id in response");
  const { id } = data;
  if (typeof id !== "number") throw new Error("id is not a number");
  return id;
};

const parseNames = (data: unknown): string[] => {
  if (!Array.isArray(data)) throw new Error("assets is not an array");
  return data.map((asset: unknown) => {
    if (typeof asset !== "object" || asset === null || !("name" in asset))
      throw new Error("missing name in asset");
    const { name } = asset;
    if (typeof name !== "string") throw new Error("name is not a string");
    return name;
  });
};

const headers = (token: string) => ({
  Authorization: `token ${token}`,
});

const MAX_ATTEMPTS = 5;
const BASE_DELAY_MS = 2000;

const isJson = (res: Response): boolean =>
  res.headers.get("content-type")?.includes("application/json") === true;

const isRetryable = (res: Response): boolean =>
  res.status >= 500 || res.status === 429 || (!res.ok && !isJson(res));

const backoff = (attempt: number): number =>
  BASE_DELAY_MS * 2 ** (attempt - 1) + Math.random() * 1000;

const fetchWithRetry = async (
  url: string,
  init: RequestInit,
  attempt = 1,
): Promise<Response> => {
  try {
    const res = await fetch(url, init);
    if (!isRetryable(res) || attempt === MAX_ATTEMPTS) return res;
    const text = await res.text();
    log.warn(`${url}: ${res.status} ${text.slice(0, 200)}`);
  } catch (error) {
    if (attempt === MAX_ATTEMPTS) throw error;
    log.warn(`${url}: ${String(error)}`);
  }
  const delay = backoff(attempt);
  log.warn(`retry ${attempt}/${MAX_ATTEMPTS - 1} in ${Math.round(delay)}ms`);
  await Bun.sleep(delay);
  return fetchWithRetry(url, init, attempt + 1);
};

const ensureRelease = async (
  tag: string,
  token: string,
  repo: string,
): Promise<number> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/tags/${tag}`;
  const res = await fetchWithRetry(url, { headers: headers(token) });

  if (res.ok) {
    const data: unknown = await res.json();
    const id = parseId(data);
    log.info(`release ${tag} already exists (id: ${id})`);
    return id;
  }

  if (res.status !== 404) {
    const text = await res.text();
    throw new Error(`check release: ${res.status} ${text}`);
  }

  log.info(`creating release ${tag}...`);
  const createRes = await fetchWithRetry(
    `https://codeberg.org/api/v1/repos/${repo}/releases`,
    {
      method: "POST",
      headers: { ...headers(token), "Content-Type": "application/json" },
      body: JSON.stringify({ tag_name: tag, name: tag }),
    },
  );

  if (!createRes.ok) {
    const text = await createRes.text();
    throw new Error(`create release: ${createRes.status} ${text}`);
  }

  const createData: unknown = await createRes.json();
  const id = parseId(createData);
  log.info(`created release ${tag} (id: ${id})`);
  return id;
};

const existingAssets = async (
  releaseId: number,
  token: string,
  repo: string,
): Promise<string[]> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/${releaseId}/assets`;
  const res = await fetchWithRetry(url, { headers: headers(token) });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`list assets: ${res.status} ${text}`);
  }

  const data: unknown = await res.json();
  return parseNames(data);
};

const upload = async (
  file: string,
  releaseId: number,
  token: string,
  repo: string,
): Promise<void> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/${releaseId}/assets?name=${file}`;
  const body = await Bun.file(file).arrayBuffer();

  const res = await fetchWithRetry(url, {
    method: "POST",
    headers: { ...headers(token), "Content-Type": "application/octet-stream" },
    body,
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`upload ${file}: ${res.status} ${text}`);
  }
};

const main = async () => {
  const tag = process.argv[2];
  if (!tag) {
    log.error("usage: bun scripts/release-upload.ts <tag>");
    process.exit(1);
  }
  if (!/^v\d+\.\d+\.\d+$/.test(tag)) {
    log.error(`invalid tag format: ${tag} (expected v0.0.0)`);
    process.exit(1);
  }

  const token = process.env["CODEBERG_TOKEN"];
  if (!token) throw new Error("CODEBERG_TOKEN is not set");

  const repo = process.env["CI_REPO"];
  if (!repo) throw new Error("CI_REPO is not set");

  const name = process.env["CI_REPO_NAME"];
  if (!name) throw new Error("CI_REPO_NAME is not set");

  const glob = new Glob(`${name}-*`);
  const entries = glob.scanSync(".");
  const files = Array.from(entries);

  if (files.length === 0) {
    log.error(`no ${name}-* files found`);
    process.exit(1);
  }

  const releaseId = await ensureRelease(tag, token, repo);
  const uploaded = await existingAssets(releaseId, token, repo);
  const pending = files.filter((file) => !uploaded.includes(file));

  if (pending.length < files.length)
    log.info(`skipping ${files.length - pending.length} already uploaded`);

  const uploadLogged = async (file: string): Promise<void> => {
    log.info(`uploading ${file}...`);
    await upload(file, releaseId, token, repo);
    log.info(`uploaded ${file}`);
  };

  const start = Promise.resolve();
  await pending.reduce(
    (previous, file) => previous.then(() => uploadLogged(file)),
    start,
  );

  log.info("release upload complete");
};

try {
  await main();
} catch (error: unknown) {
  const message = String(error);
  log.error(message);
  process.exitCode = 1;
}
