import { accessSync, constants } from "fs";
import { homedir } from "os";
import { join } from "path";
import { log } from "./lib/log.ts";

const home = homedir();

const blockedPaths = [
  "/etc/passwd",
  "/etc/shadow",
  "/root",

  join(home, ".ssh/id_ed25519"),
  join(home, ".ssh/id_rsa"),
  join(home, ".ssh/id_ecdsa"),
  join(home, ".ssh/config"),
  join(home, ".ssh/authorized_keys"),

  join(home, ".git-credentials"),
  join(home, ".config/sops"),

  join(home, ".aws/credentials"),
  join(home, ".aws/config"),
  join(home, ".kube/config"),
  join(home, ".config/gcloud/application_default_credentials.json"),
  join(home, ".azure"),
  join(home, ".docker/config.json"),

  join(home, ".npmrc"),
  join(home, ".vault-token"),
  join(home, ".netrc"),
  join(home, ".my.cnf"),
  join(home, ".pgpass"),

  join(home, ".bash_history"),
  join(home, ".zsh_history"),

  join(home, ".bitcoin"),
  join(home, ".ethereum/keystore"),

  "/etc/ssl/private",
  "/etc/kubernetes/admin.conf",
  "/var/lib",
];

const warnPaths = [
  join(home, ".cargo/credentials.toml"),
];

const checkBlocked = (path: string): { pass: boolean; msg: string } => {
  try {
    accessSync(path, constants.R_OK);
    return { pass: false, msg: `${path} — readable` };
  } catch {
    return { pass: true, msg: `${path} — blocked` };
  }
};

const checkWarn = (path: string): { warn: boolean; msg: string } => {
  try {
    accessSync(path, constants.R_OK);
    return { warn: true, msg: `${path} — readable (review if sensitive)` };
  } catch {
    return { warn: false, msg: `${path} — blocked` };
  }
};

const blockedResults = blockedPaths.map(checkBlocked);
blockedResults.forEach((r) => (r.pass ? log.ok(r.msg) : log.error(r.msg)));

const warnResults = warnPaths.map(checkWarn);
warnResults.forEach((r) => (r.warn ? log.warn(r.msg) : log.ok(r.msg)));

const failures = blockedResults.filter((r) => !r.pass);
if (failures.length > 0) {
  process.exit(1);
}
