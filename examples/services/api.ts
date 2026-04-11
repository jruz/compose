import { log } from "../../scripts/log";

const PORT = 8555;
const PG = "http://localhost:5555";
const REDIS = "http://localhost:6555";

type Migration = string;
const MIGRATIONS: readonly Migration[] = ["001_create_users", "002_create_orders", "003_add_indexes"];

type Route = Readonly<{ method: string; path: string }>;
const ROUTES: readonly Route[] = [
  { method: "GET", path: "/api/v1/users" },
  { method: "GET", path: "/api/v1/users/:id" },
  { method: "POST", path: "/api/v1/users" },
  { method: "GET", path: "/api/v1/orders" },
  { method: "POST", path: "/api/v1/orders" },
];

const runMigrations = async () => {
  log.info("running migrations...");
  const initialPromise = Promise.resolve();
  await MIGRATIONS.reduce(async (prev, m) => {
    await prev;
    await Bun.sleep(200);
    log.info(`  applied ${m}`);
  }, initialPromise);
  log.info(`migrations complete (${MIGRATIONS.length} applied)`);
};

const handleRequest = async (req: Request): Promise<Response> => {
  const start = performance.now();
  const url = new URL(req.url);

  if (url.pathname === "/health") {
    return new Response("ok");
  }

  try {
    const cacheKey = `cache:${url.pathname}`;
    const cached = await fetch(`${REDIS}/get/${cacheKey}`);

    if (cached.status === 200) {
      const body = await cached.text();
      const ms = (performance.now() - start).toFixed(1);
      log.info(`${req.method} ${url.pathname} 200 ${ms}ms (cache hit)`);
      return new Response(body, { headers: { "x-cache": "HIT" } });
    }

    await fetch(`${PG}/health`);

    const body = JSON.stringify({ path: url.pathname, ts: Date.now() });
    await fetch(`${REDIS}/set/${cacheKey}`, { method: "POST", body });

    const ms = (performance.now() - start).toFixed(1);
    log.info(`${req.method} ${url.pathname} 200 ${ms}ms`);
    return new Response(body, { headers: { "content-type": "application/json" } });
  } catch (e) {
    const ms = (performance.now() - start).toFixed(1);
    const msg = e instanceof Error ? e.message : String(e);
    log.error(`${req.method} ${url.pathname} 502 ${ms}ms - ${msg}`);
    return new Response("bad gateway", { status: 502 });
  }
};

const main = async () => {
  log.info("api-server v2.1.0");

  await runMigrations();

  log.info(`connecting to postgres at ${PG}`);
  log.info(`connecting to redis at ${REDIS}`);
  log.info(`routes loaded: ${ROUTES.length} endpoints`);

  const server = Bun.serve({
    port: PORT,
    fetch: handleRequest,
  });

  log.info(`listening on :${PORT}`);

  process.on("SIGTERM", () => {
    log.info("received SIGTERM, draining connections...");
    server.stop();
    log.info("shutdown complete");
    process.exit(0);
  });
};

await main();
