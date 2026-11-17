use lib/log.nu

const LAYERS = {
    commands: {calls: [core utils], siblings: false}
    core: {calls: [libs utils], siblings: true}
    libs: {calls: [utils], siblings: false}
    utils: {calls: [], siblings: false}
}

const ENTRYPOINT_CALLS = [commands utils]

def file-calls [pattern: string]: path -> table {
    let file = ($in | path expand)
    (open $file
        | lines
        | enumerate
        | each {|row|
            $row.item
                | parse --regex $pattern
                | insert file ($file | path relative-to $env.PWD)
                | insert line ($row.index + 1)
        }
        | flatten)
}

def owner [layer_root: path]: path -> string {
    path relative-to $layer_root | path split | first | str replace --regex '\.rs$' ''
}

def is-allowed [layer: record, module: string]: record -> bool {
    let call = $in
    if $LAYERS not-has $call.target {
        return true
    }
    if $call.target == $layer.name {
        return ($layer.rule.siblings or $call.module == $module)
    }
    $call.target in $layer.rule.calls
}

def layer-violations []: record -> list<string> {
    let layer = $in
    (glob ($layer.root | path join '**/*.rs')
        | each {|file|
            let module = ($file | owner $layer.root)
            ($file
                | file-calls 'crate::(?<target>\w+)::(?<module>\w+)'
                | where not ($it | is-allowed $layer $module))
        }
        | flatten
        | each {|call| $"($call.file):($call.line): ($layer.name) cannot call ($call.target)::($call.module)" })
}

def entrypoint-violations [entrypoint: path]: nothing -> list<string> {
    if not ($entrypoint | path exists) {
        return []
    }
    ($entrypoint
        | file-calls '\b(?<target>commands|core|libs|utils)::'
        | where target not-in $ENTRYPOINT_CALLS
        | each {|call| $"($call.file):($call.line): main cannot call ($call.target)" })
}

# Check that each layer only calls the layers it depends on
def main [
    source: path # source directory containing the layer folders
]: nothing -> nothing {
    let violations = ($LAYERS
        | items {|name, rule| {root: ($source | path join $name | path expand), name: $name, rule: $rule} }
        | where ($it.root | path exists)
        | each { layer-violations }
        | flatten
        | prepend (entrypoint-violations ($source | path join main.rs)))
    if ($violations | is-not-empty) {
        for violation in $violations {
            log error $violation
        }
        exit 1
    }
    log ok "layers only call the layers they depend on"
}
