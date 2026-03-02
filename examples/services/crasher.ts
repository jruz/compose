import { log } from "../../scripts/log";

const main = async () => {
  log.info("crasher starting");
  log.info("connecting to database...");
  await Bun.sleep(1000);
  log.error("FATAL: connection refused to localhost:5432");
  process.exit(1);
};

await main();
