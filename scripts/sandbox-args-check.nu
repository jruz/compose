use log.nu

def run-sandbox [args: list<string>, env_vars: record] {
    with-env $env_vars {
        "" | run-external sandbox ...$args | complete
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
    let dir = mktemp --directory sandbox-args-check.XXXXXXXXXX | path expand
    let path = $dir | path join $name
    $body | save $path
    ^chmod u+x $path
    let result = do $use $path
    rm --recursive $dir
    $result
}

def check-argument-boundaries [] {
    let args = ["printf" "<%s>\n" "hello world" "" "*.nix" "*" "?" "[ab]" 'quote "with" spaces']
    let outcome = run-sandbox $args {}
    let expected = "<hello world>\n<>\n<*.nix>\n<*>\n<?>\n<[ab]>\n<quote \"with\" spaces>\n"
    expect-stdout "preserves argument boundaries" $outcome $expected
}

def check-executable-path-with-spaces [] {
    with-executable "command with spaces" "#!/bin/sh\nexec printf \"<%s>\\n\" \"$@\"\n" {|path|
        let outcome = run-sandbox [$path "hello world" "" "*.nix"] {}
        expect-stdout "runs executable paths with spaces" $outcome "<hello world>\n<>\n<*.nix>\n"
    }
}

def check-selected-shell [] {
    with-executable "shell with spaces" "#!/bin/sh\nprintf \"selected-shell\\n\"\n" {|path|
        let outcome = run-sandbox [] {SHELL: $path}
        expect-stdout "runs the selected shell without arguments" $outcome "selected-shell\n"
    }
}

def check-empty-command [] {
    let outcome = run-sandbox [""] {SHELL: "/bin/sh"}
    expect-failure "rejects an empty command" $outcome "execvp :"
}

def check-empty-shell [] {
    let outcome = run-sandbox [] {SHELL: ""}
    let diagnostic = "SHELL must be set when sandbox runs without a command"
    expect-failure "rejects an empty shell" $outcome $diagnostic
}

def check-command-is-not-bubblewrap-option [] {
    let outcome = run-sandbox ["--help"] {}
    expect-failure "does not treat the command as a bubblewrap option" $outcome "execvp --help:"
}

def main [] {
    let results = [
        (check-argument-boundaries)
        (check-executable-path-with-spaces)
        (check-selected-shell)
        (check-empty-command)
        (check-empty-shell)
        (check-command-is-not-bubblewrap-option)
    ]
    for result in $results {
        if $result.pass { log ok $result.msg } else { log err $result.msg }
    }
    if ($results | any {|result| not $result.pass }) {
        exit 1
    }
}
