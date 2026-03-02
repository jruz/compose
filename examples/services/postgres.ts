import { log } from "../../scripts/log";

const PORT = 5555;

const main = () => {
  log.info(`PostgreSQL 16.2 starting on port ${PORT}`);
  log.info("loading configuration files");
  log.info("initializing shared memory");

  const server = Bun.serve({
    port: PORT,
    fetch(req) {
      const url = new URL(req.url);
      if (url.pathname === "/health") {
        return new Response("ok");
      }
      return new Response("not found", { status: 404 });
    },
  });

  log.info(`listening on IPv4 address 0.0.0.0, port ${PORT}`);
  log.info("database system is ready to accept connections");

  process.on("SIGTERM", () => {
    log.info("received fast shutdown request");
    log.info("shutting down");
    server.stop();
    process.exit(0);
  });
};

main();
