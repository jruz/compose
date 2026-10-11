use log.nu

def check-command [wrapper: string, args: list<string>, label: string] {
    let outcome = (run-external $wrapper ...$args | complete)
    let pass = $outcome.exit_code == 0
    let detail = if $pass { "passed" } else { $"exit ($outcome.exit_code): ($outcome.stderr)" }
    {pass: $pass, msg: $"($label) — ($detail)"}
}

def main [] {
    let socket = "/var/run/docker.sock"
    let results = [
        (check-command "sandbox" ["test" "!" "-e" $socket] "regular sandbox hides Docker socket")
        (check-command "sandbox-docker" ["test" "-S" $socket] "Docker opt-in exposes socket")
        (check-command "sandbox-docker" ["docker" "--host" $"unix://($socket)" "info" "--format" "{{.ServerVersion}}"] "Docker opt-in reaches host daemon")
    ]
    for result in $results {
        if $result.pass { log ok $result.msg } else { log err $result.msg }
    }
    if ($results | any {|result| not $result.pass }) {
        exit 1
    }
}
