import { log } from "./log";

const headers = (token: string) => ({
  Authorization: `token ${token}`,
});

const run = async (cmd: string[]): Promise<{ ok: boolean; stdout: string }> => {
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(proc.stdout).text();
  const code = await proc.exited;
  return { ok: code === 0, stdout: stdout.trim() };
};

const deleteLocalTag = async (tag: string): Promise<void> => {
  const list = await run(["git", "tag", "--list", tag]);
  if (list.stdout === "") {
    log.info(`local tag ${tag} not found, skipping`);
    return;
  }
  const result = await run(["git", "tag", "--delete", tag]);
  if (!result.ok) throw new Error(`git tag --delete ${tag} failed`);
  log.info(`deleted local tag ${tag}`);
};

const deleteRemoteTag = async (tag: string): Promise<void> => {
  const list = await run(["git", "ls-remote", "--tags", "origin", `refs/tags/${tag}`]);
  if (list.stdout === "") {
    log.info(`remote tag ${tag} not found, skipping`);
    return;
  }
  const result = await run(["git", "push", "origin", `:refs/tags/${tag}`]);
  if (!result.ok) throw new Error(`git push origin :refs/tags/${tag} failed`);
  log.info(`deleted remote tag ${tag}`);
};

const findRelease = async (tag: string, token: string, repo: string): Promise<number | null> => {
  const url = `https://codeberg.org/api/v1/repos/${repo}/releases/tags/${tag}`;
  const res = await fetch(url, { headers: headers(token) });

  if (res.ok) {
    const data = await res.json();
    return data.id;
  }

  if (res.status === 404) {
    return null;
  }

  const text = await res.text();
  throw new Error(`check release: ${res.status} ${text}`);
};

const deleteRelease = async (releaseId: number, token: string, repo: string): Promise<void> => {
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

  const token = process.env.CODEBERG_TOKEN;
  if (!token) throw new Error("CODEBERG_TOKEN is not set");

  const repo = process.env.CI_REPO ?? "jruz/compose";

  await deleteLocalTag(tag);
  await deleteRemoteTag(tag);

  const releaseId = await findRelease(tag, token, repo);
  if (releaseId === null) {
    log.info(`no release found for ${tag}`);
  } else {
    await deleteRelease(releaseId, token, repo);
    log.info(`deleted release ${tag} (id: ${releaseId})`);
  }
};

await main();
