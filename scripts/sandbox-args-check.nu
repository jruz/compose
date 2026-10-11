use log.nu

def run-sandbox [wrapper: string, args: list<string>, env_vars: record] {
    with-env $env_vars {
        "" | run-external $wrapper ...$args | complete
    }
}

def describe [outcome: record] {
    let stdout = $outcome.stdout | to json --raw
    let stderr = $outcome.stderr | to json --raw
    $"exit ($outcome.exit_code), stdout ($stdout), stderr ($stderr)"
}

def to-result [label: string, pass: bool, outcome: record] {
    let detail = if $pass { "passed" } else { describe $outcome }
    {pass: $pass, msg: $"($label) — ($detail)"}
}

def expect-stdout [label: string, outcome: record, expected: string] {
    let pass = $outcome.exit_code == 0 and $outcome.stdout == $expected
    to-result $label $pass $outcome
}

def expect-failure [label: string, outcome: record, diagnostic: string] {
    let pass = $outcome.exit_code != 0 and ($outcome.stderr | str contains $diagnostic)
    to-result $label $pass $outcome
}

def with-executable [name: string, body: string, use: closure] {
    let dir = ^mktemp --directory --tmpdir=. sandbox-args-check.XXXXXXXXXX | path expand
    let path = $dir | path join $name
    $body | save $path
    ^chmod u+x $path
    let result = do $use $path
    rm --recursive $dir
    $result
}

def check-argument-boundaries [wrapper: string] {
    let args = ["printf" "<%s>\n" "hello world" "" "*.nix" "*" "?" "[ab]" 'quote "with" spaces']
    let outcome = run-sandbox $wrapper $args {}
    let expected = "<hello world>\n<>\n<*.nix>\n<*>\n<?>\n<[ab]>\n<quote \"with\" spaces>\n"
    expect-stdout "preserves argument boundaries" $outcome $expected
}

def check-executable-path-with-spaces [wrapper: string] {
    with-executable "command with spaces" "#!/bin/sh\nexec printf \"<%s>\\n\" \"$@\"\n" {|path|
        let outcome = run-sandbox $wrapper [$path "hello world" "" "*.nix"] {}
        expect-stdout "runs executable paths with spaces" $outcome "<hello world>\n<>\n<*.nix>\n"
    }
}

def check-missing-command [wrapper: string] {
    let outcome = run-sandbox $wrapper [] {}
    expect-failure "rejects a missing command" $outcome "[OPTIONS...] [--] COMMAND [ARGS...]"
}

def check-empty-command [wrapper: string] {
    let outcome = run-sandbox $wrapper [""] {}
    expect-failure "rejects an empty command" $outcome "execvp :"
}

def check-command-is-not-bubblewrap-option [wrapper: string] {
    let outcome = run-sandbox $wrapper ["--help"] {}
    expect-failure "does not treat the command as a bubblewrap option" $outcome "execvp --help:"
}

def main [wrapper: string] {
    let results = [
        (check-argument-boundaries $wrapper)
        (check-executable-path-with-spaces $wrapper)
        (check-missing-command $wrapper)
        (check-empty-command $wrapper)
        (check-command-is-not-bubblewrap-option $wrapper)
    ]
    for result in $results {
        let msg = $"($wrapper): ($result.msg)"
        if $result.pass { log ok $msg } else { log err $msg }
    }
    if ($results | any {|result| not $result.pass }) {
        exit 1
    }
}
