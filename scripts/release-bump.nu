def "log err" [msg: string]: nothing -> nothing {
    print $"[(ansi red)ERROR(ansi reset)] ($msg)"
}

def fail [label: string, stderr: string]: nothing -> error {
    let trimmed = ($stderr | str trim)
    let message = if ($trimmed | is-empty) {
        $"($label) failed"
    } else {
        $"($label) failed: ($trimmed)"
    }
    error make {msg: $message}
}

def cargo-set-version [version: string]: nothing -> nothing {
    let result = (^sandbox cargo set-version $version | complete)
    if $result.exit_code != 0 {
        fail "cargo set-version" $result.stderr
    }
}

def git-add-release-files []: nothing -> nothing {
    let result = (^git add Cargo.toml Cargo.lock | complete)
    if $result.exit_code != 0 {
        fail "git add" $result.stderr
    }
}

def git-commit-version [version: string]: nothing -> nothing {
    let result = (^git commit --message $"[tool] Version: Bump to ($version)" | complete)
    if $result.exit_code != 0 {
        fail "git commit" $result.stderr
    }
}

def main [version: string]: nothing -> nothing {
    if ($version !~ '^\d+\.\d+\.\d+$') {
        log err $"invalid version format: ($version) \(expected 0.0.0\)"
        exit 1
    }

    try {
        let current = (open Cargo.toml | get package.version)
        cargo-set-version $version
        git-add-release-files
        git-commit-version $version
        print $"[(ansi green)OK(ansi reset)] Bumped ($current) -> ($version)"
    } catch {|err|
        log err $err.msg
        exit 1
    }
}
