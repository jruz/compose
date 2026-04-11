import { log } from "../../scripts/log";

const API = "http://localhost:8555";
type Path = string;
const PATHS: readonly Path[] = ["/api/v1/users", "/api/v1/users/42", "/api/v1/orders"];
const TOTAL = 15;

const sendRequest = async (index: number): Promise<void> => {
  const path = PATHS[index % PATHS.length];
  const delay = 300 + Math.random() * 700;
  await Bun.sleep(delay);

  try {
    const res = await fetch(`${API}${path}`);
    log.info(`${res.status} ${path}`);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log.error(`ERR ${path} - ${msg}`);
  }
};

const main = async () => {
  log.info(`traffic generator: ${TOTAL} requests to ${API}`);

  const indices = Array.from({ length: TOTAL }, (_, i) => i);
  const initial = Promise.resolve();
  await indices.reduce(
    async (prev, i) => {
      await prev;
      await sendRequest(i);
    },
    initial,
  );

  log.info("traffic generation complete");
  process.exit(0);
};

await main();
