import { log } from "./lib/log.ts";

const parseId = (data: unknown): number => {
  if (typeof data !== "object" || data === null || !("id" in data))
    throw new Error("missing id in response");
  const { id } = data;
  if (typeof id !== "number") throw new Error("id is not a number");
  return id;
};

const headers = (token: string) => ({
  Authorization: `token ${token}`,
});

const run = async (cmd: string[]): Promise<{ ok: boolean; stdout: string }> => {
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(proc.stdout).text();
  const code = await proc.exited;
  return { ok: code === 0, stdout: stdout.trim() };
};

const repoFromRemote = async (): Promise<string> => {
  const result = await run(["git", "remote", "get-url", "origin"]);
  if (!result.ok) throw new Error("failed to get origin remote URL");
  const match = /codeberg\.org[:/](.+?)(?:\.git)?$/.exec(result.stdout);
  if (!match?.[1])
    throw new Error(`cannot parse repo from remote: ${result.stdout}`);
  return match[1];
};

const deleteRemoteTag = async (
  tag: string,
  token: string,
  repo: string,
): Promise<void> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/tags/${tag}`;
  const res = await fetch(url, { headers: headers(token) });

  if (res.status === 404) {
    log.info(`remote tag ${tag} not found, skipping`);
    return;
  }

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`check tag: ${res.status} ${text}`);
  }

  const del = await fetch(url, { method: "DELETE", headers: headers(token) });
  if (!del.ok) {
    const text = await del.text();
    throw new Error(`delete tag: ${del.status} ${text}`);
  }

  log.info(`deleted remote tag ${tag}`);
};

const findRelease = async (
  tag: string,
  token: string,
  repo: string,
): Promise<number | null> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/tags/${tag}`;
  const res = await fetch(url, { headers: headers(token) });

  if (res.ok) {
    const data: unknown = await res.json();
    return parseId(data);
  }

  if (res.status === 404) {
    return null;
  }

  const text = await res.text();
  throw new Error(`check release: ${res.status} ${text}`);
};

const deleteRelease = async (
  releaseId: number,
  token: string,
  repo: string,
): Promise<void> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/${releaseId}`;
  const res = await fetch(url, {
    method: "DELETE",
    headers: headers(token),
  });

  if (!res.ok) {
    const text = await res.text();
    throw new Error(`delete release: ${res.status} ${text}`);
  }
};

const main = async () => {
  const tag = process.argv[2];
  if (!tag) {
    log.error("usage: bun scripts/release-delete.ts <tag>");
    process.exit(1);
  }
  if (!/^v\d+\.\d+\.\d+$/.test(tag)) {
    log.error(`invalid tag format: ${tag} (expected v0.0.0)`);
    process.exit(1);
  }

  const token = process.env["CODEBERG_TOKEN"];
  if (!token) throw new Error("CODEBERG_TOKEN is not set");

  const repo = await repoFromRemote();

  const releaseId = await findRelease(tag, token, repo);
  if (releaseId === null) {
    log.info(`no release found for ${tag}`);
  } else {
    await deleteRelease(releaseId, token, repo);
    log.info(`deleted release ${tag} (id: ${releaseId})`);
  }

  await deleteRemoteTag(tag, token, repo);
};

try {
  await main();
} catch (error: unknown) {
  const message = String(error);
  log.error(message);
  process.exitCode = 1;
}
