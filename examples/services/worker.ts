import { log } from "../../scripts/log";

const REDIS = "http://localhost:6555";
type Queue = string;
const QUEUES: readonly Queue[] = ["emails", "notifications", "reports"];

const createShutdownSignal = (): (() => boolean) => {
  let stopped = false;
  process.on("SIGTERM", () => {
    log.info("received SIGTERM, finishing current job...");
    stopped = true;
  });
  return () => stopped;
};

const processJob = async (jobCount: number, shouldStop: () => boolean): Promise<number> => {
  if (shouldStop()) return jobCount;

  const delay = 500 + Math.random() * 1500;
  await Bun.sleep(delay);

  if (shouldStop()) return jobCount;

  const next = jobCount + 1;
  const queue = QUEUES[next % QUEUES.length];
  const start = performance.now();

  try {
    const key = `job:${queue}:${next}`;
    await fetch(`${REDIS}/set/${key}`, {
      method: "POST",
      body: JSON.stringify({ queue, job: next, ts: Date.now() }),
    });

    const ms = (performance.now() - start).toFixed(1);
    log.info(`processed job #${next} from ${queue} queue (${ms}ms)`);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    log.error(`job #${next} from ${queue} failed: ${msg}`);
  }

  return processJob(next, shouldStop);
};

const main = async () => {
  log.info("worker starting");
  log.info(`subscribed to queues: ${QUEUES.join(", ")}`);

  const shouldStop = createShutdownSignal();
  const totalJobs = await processJob(0, shouldStop);

  log.info(`worker stopped after ${totalJobs} jobs`);
  process.exit(0);
};

await main();
