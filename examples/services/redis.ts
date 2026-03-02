import { log } from "../../scripts/log";

const PORT = 6555;

const main = () => {
  log.info("Redis 7.2.4 starting");
  log.info("Server initialized");

  const store = new Map<string, string>();

  const server = Bun.serve({
    port: PORT,
    async fetch(req) {
      const url = new URL(req.url);

      if (url.pathname === "/health") {
        return new Response("PONG");
      }

      if (req.method === "GET" && url.pathname.startsWith("/get/")) {
        const key = url.pathname.slice(5);
        const val = store.get(key);
        if (val === undefined) {
          return new Response("(nil)", { status: 404 });
        }
        return new Response(val);
      }

      if (req.method === "POST" && url.pathname.startsWith("/set/")) {
        const key = url.pathname.slice(5);
        const val = await req.text();
        store.set(key, val);
        log.info(`SET ${key} -> ${val}`);
        return new Response("OK");
      }

      return new Response("ERR unknown command", { status: 400 });
    },
  });

  log.info(`Ready to accept connections tcp *:${PORT}`);

  process.on("SIGTERM", () => {
    log.info("User requested shutdown...");
    log.info("Redis is now ready to exit, bye bye...");
    server.stop();
    process.exit(0);
  });
};

main();
