use log.nu

def main [version: string] {
    if ($version !~ '^\d+\.\d+\.\d+$') {
        log err $"invalid version format: ($version) \(expected 0.0.0\)"
        exit 1
    }
    let current = (open Cargo.toml | get package.version)
    ^cargo set-version $version
    ^cargo generate-lockfile
    ^git add Cargo.toml Cargo.lock
    ^git commit --message $"[tool] Version: Bump to ($version)"
    log ok $"Bumped ($current) -> ($version)"
}
