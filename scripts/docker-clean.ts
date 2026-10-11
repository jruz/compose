import { log } from "./lib/log.ts";

const run = async (
  cmd: string[]
): Promise<{ ok: boolean; stdout: string }> => {
  const proc = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
  const stdout = await new Response(proc.stdout).text();
  const code = await proc.exited;
  return { ok: code === 0, stdout: stdout.trim() };
};

const lines = (output: string): string[] =>
  output.split("\n").filter((l) => l !== "");

const removeAll = (ids: string[], cmd: (id: string) => string[]) => {
  const runs = ids.map((id) => {
    const args = cmd(id);
    return run(args);
  });
  return Promise.all(runs);
};

const cleanContainers = async () => {
  const result = await run([
    "docker",
    "ps",
    "--all",
    "--filter=name=compose-",
    "--quiet",
  ]);
  const ids = lines(result.stdout);
  if (ids.length === 0) {
    log.info("no compose-* containers found");
    return;
  }
  await removeAll(ids, (id) => ["docker", "rm", "--force", id]);
  log.ok(`removed ${ids.length} containers`);
};

const cleanImages = async () => {
  const result = await run([
    "docker",
    "images",
    "--filter=reference=compose-test-*",
    "--quiet",
  ]);
  const ids = lines(result.stdout);
  if (ids.length === 0) {
    log.info("no compose-test-* images found");
    return;
  }
  await removeAll(ids, (id) => ["docker", "rmi", id]);
  log.ok(`removed ${ids.length} images`);
};

const cleanNetworks = async () => {
  const result = await run([
    "docker",
    "network",
    "ls",
    "--format={{.Name}}",
  ]);
  const nets = lines(result.stdout).filter((n) =>
    n.startsWith("compose-")
  );
  if (nets.length === 0) {
    log.info("no compose-* networks found");
    return;
  }
  await removeAll(nets, (n) => ["docker", "network", "rm", n]);
  log.ok(`removed ${nets.length} networks`);
};

await cleanContainers();
await cleanImages();
await cleanNetworks();
