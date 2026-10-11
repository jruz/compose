use log.nu

def check-private-tmp [] {
    let dir = ^mktemp --directory --tmpdir=/tmp sandbox-tmp-check.XXXXXXXXXX | str trim
    let marker = $dir | path join "host-only"
    ^touch $marker
    let outcome = (^sandbox test "!" "-e" $marker | complete)
    ^rm --recursive $dir
    let pass = $outcome.exit_code == 0
    let detail = if $pass { "hidden" } else { $"visible or check failed; exit ($outcome.exit_code): ($outcome.stderr)" }
    {pass: $pass, msg: $"host /tmp marker — ($detail)"}
}

def main [] {
    let result = check-private-tmp
    if $result.pass {
        log ok $result.msg
    } else {
        log err $result.msg
        exit 1
    }
}
