use lib/log.nu
use lib/targets.nu [TARGETS]


def cargo-set-version [version: string]: nothing -> nothing {
    ^sandbox cargo set-version $version
}


def valid-version [version: string]: nothing -> bool {
    $version =~ '^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
}


def bumped-version [version: string, bump: string]: nothing -> string {
    let parts = ($version | split row '.' | each {|part| $part | into int })
    match $bump {
        major => $"($parts.0 + 1).0.0"
        minor => $"($parts.0).($parts.1 + 1).0"
        patch => $"($parts.0).($parts.1).($parts.2 + 1)"
    }
}


# Bump the version on origin/main and start a Woodpecker release
def "main bump" [
    kind: string # SemVer bump: patch, minor, or major
]: nothing -> nothing {
    if $kind not-in [patch minor major] {
        log error 'Expected patch, minor, or major'
        exit 1
    }
    let origin = (^git config --get remote.origin.url | str trim)
    let repos = ($origin
        | str replace --regex '\.git/?$' ''
        | parse --regex '^(?:https://github\.com/|ssh://git@github\.com/|git@github\.com:)(?<repo>[\w.-]+/[\w.-]+)/?$')
    if ($repos | is-empty) {
        log error 'Expected origin to point to a GitHub repository'
        exit 1
    }
    let repo = ($repos | first | get repo)
    ^git fetch --no-tags origin main
    let manifest = (^git show FETCH_HEAD:Cargo.toml)
    let current = ($manifest | from toml | get package.version)
    if not (valid-version $current) {
        log error $"Invalid version ($current) on origin/main; expected 0.0.0"
        exit 1
    }
    let version = (bumped-version $current $kind)
    log info $"Starting release ($version) for ($repo)"
    ^woodpecker-cli pipeline create --branch main --var $"RELEASE_VERSION=($version)" $repo
}


def find-release [tag: string]: nothing -> list<record> {
    let response = (^gh api --paginate --slurp 'repos/{owner}/{repo}/releases')
    $response | from json | flatten | where tag_name == $tag
}


def upload-draft [release: record, artifacts: list<path>]: nothing -> nothing {
    if (find-release $release.draft | is-empty) {
        ^gh release create $release.draft --draft --title $release.tag --notes ''
    }
    ^gh release upload $release.draft ...$artifacts --clobber
}


def is-manifest-current [version: string]: nothing -> bool {
    if not ('release.json' | path exists) {
        return false
    }
    (open release.json | get version) == $version
}


def commit-manifest [version: string]: nothing -> nothing {
    ^git add -- Cargo.toml Cargo.lock release.json
    ^git commit --only --message $"[tool] Release: ($version)" -- Cargo.toml Cargo.lock release.json
}


def checkout-main []: nothing -> nothing {
    ^git fetch --no-tags origin main
    ^git switch --detach FETCH_HEAD
}


def push-main []: nothing -> record {
    let output = (^sh -c 'git push --porcelain origin HEAD:refs/heads/main; printf "%s\n" "$?"')
    let lines = ($output | lines)
    let status = ($lines | last | into int)
    let details = ($lines | drop | str join (char newline))
    let advanced = ($details =~ '(?m)^!\tHEAD:refs/heads/main\t\[rejected\] \((fetch first|non-fast-forward)\)')
    {status: $status, advanced: $advanced, details: $details}
}


def head-commit []: nothing -> string {
    ^git rev-parse HEAD | str trim
}


def manifest-commit []: nothing -> string {
    let shallow = (^git rev-parse --is-shallow-repository | str trim)
    if $shallow == 'true' {
        ^git fetch --unshallow --no-tags origin
    }
    ^git log --first-parent --max-count=1 --format=%H HEAD -- release.json | str trim
}


def tag-available [release: record]: nothing -> bool {
    let tag_ref = $"refs/tags/($release.tag)"
    let refs = (^git ls-remote origin $tag_ref $"($tag_ref)^{}")
    if ($refs | is-empty) or (($refs | lines | last | split row (char tab) | first) == $release.commit) {
        return true
    }
    log error $"Tag ($release.tag) already points to a different commit"
    false
}


def publish-draft [release: record]: nothing -> int {
    if not (tag-available $release) {
        return 1
    }
    ^gh release edit $release.draft --tag $release.tag --draft=false --target $release.commit
    log ok $"published ($release.tag) at ($release.commit)"
    0
}


def release-platform [release: record, target: record]: nothing -> record {
    let asset_name = $"($release.name)-($target.os)-($target.arch)"
    let asset = ($release.assets | where name == $asset_name | first)
    let digest = ($asset.digest | parse --regex '^sha256:(?<hex>[0-9a-f]{64})$' | first)
    let hash = ($digest.hex | decode hex | encode base64)
    let os = if $target.os == unknown-linux-musl { 'linux' } else { 'darwin' }
    {
        system: $"($target.arch)-($os)"
        platform: {asset: $asset_name, hash: $"sha256-($hash)"}
    }
}


def release-manifest [version: string, release: record]: nothing -> record {
    let platforms = ($TARGETS
        | each {|target| release-platform $release $target }
        | each {|item| {$item.system: $item.platform} }
        | into record)
    {version: $version, platforms: $platforms}
}


def write-manifest [manifest: record]: nothing -> nothing {
    ($manifest
        | to json --indent 2
        | $"($in)\n"
        | save --force release.json)
}


# Generate release.json from the latest GitHub release assets without committing
def "main manifest-generate" []: nothing -> nothing {
    let response = (^gh api 'repos/{owner}/{repo}/releases/latest')
    let published = ($response | from json)
    let version = ($published.tag_name
        | parse --regex '^v(?<version>\d+\.\d+\.\d+)$'
        | first
        | get version)
    let name = open Cargo.toml | get package.name
    write-manifest (release-manifest $version {name: $name, assets: $published.assets})
    log ok $"release manifest generated: ($published.tag_name)"
}


def draft-matches [release: record, manifest: record]: nothing -> bool {
    let drafts = (find-release $release.draft)
    if ($drafts | is-empty) {
        log error $"GitHub draft ($release.draft) is missing; cannot publish"
        return false
    }
    let draft = ($drafts | first)
    if $draft.draft and (release-manifest $release.version {name: $release.name, assets: $draft.assets}) == $manifest {
        return true
    }
    log error $"Draft ($release.draft) does not match release.json; cannot publish"
    false
}


def resume-release [request: record, releases: list<record>]: nothing -> int {
    let commit = (manifest-commit)
    let draft = if ($releases | is-empty) { $"($request.tag)-draft-($commit)" } else { $request.tag }
    let release = ($request | merge {commit: $commit, draft: $draft})
    let manifest = (^git show $"($commit):release.json")
    if not (draft-matches $release ($manifest | from json)) {
        return 1
    }
    publish-draft $release
}


def is-older [version: string, current: string]: nothing -> bool {
    let differences = ($version | split row '.' | zip ($current | split row '.')
        | each {|pair| ($pair.0 | into int) - ($pair.1 | into int) }
        | where $it != 0)
    if ($differences | is-empty) {
        return false
    }
    ($differences | first) < 0
}


def build-artifacts [name: string]: nothing -> list<path> {
    with-env {REVISION: (head-commit)} {
        ^nix develop --no-update-lock-file --command just check test
        ^nu scripts/build.nu all release
    }
    $TARGETS | each {|target| $"target/dist/($name)-($target.os)-($target.arch)" }
}


def attempt-release [version: string]: nothing -> int {
    checkout-main
    let package = (open Cargo.toml | get package)
    let name = $package.name
    let tag = $"v($version)"
    let releases = (find-release $tag)
    if ($releases | any {|release| not $release.draft }) {
        log info $"($tag) already published, skipping"
        return 0
    }
    let request = {version: $version, name: $name, tag: $tag}
    if (is-manifest-current $version) {
        return (resume-release $request $releases)
    }
    if (is-older $version $package.version) {
        log error $"($version) is older than ($package.version) on main"
        return 1
    }
    if $package.version != $version {
        cargo-set-version $version
    }
    build-release $request
}


def build-release [request: record]: nothing -> int {
    let artifacts = (build-artifacts $request.name)
    let assets = ($artifacts | each {|artifact|
        let digest = (open --raw $artifact | hash sha256)
        {name: ($artifact | path basename), digest: $"sha256:($digest)"}
    })
    let manifest = (release-manifest $request.version {name: $request.name, assets: $assets})
    write-manifest $manifest
    commit-manifest $request.version
    let commit = (head-commit)
    let release = ($request | merge {commit: $commit, draft: $"($request.tag)-draft-($commit)"})
    if not (tag-available $release) {
        return 1
    }
    upload-draft $release $artifacts
    if not (draft-matches $release $manifest) {
        return 1
    }
    let push = (push-main)
    if $push.advanced {
        ^gh release delete $release.draft --yes
        return (-1)
    }
    if $push.status != 0 {
        log error $"Could not push the release commit to main: ($push.details)"
        return $push.status
    }
    publish-draft $release
}


# Build and publish a version from main
def "main publish" [
    version: string # requested version in 0.0.0 format
]: nothing -> nothing {
    if not (valid-version $version) {
        log error $"Invalid version ($version); expected 0.0.0"
        exit 1
    }
    let status = (^git status --porcelain --untracked-files=no)
    if ($status | is-not-empty) {
        log error 'Release requires a clean checkout'
        exit 1
    }
    let first = (attempt-release $version)
    if $first != -1 {
        exit $first
    }
    log info 'main advanced; rebuilding from the latest main'
    let retry = (attempt-release $version)
    if $retry == -1 {
        log error 'main advanced again; start a new release run to retry'
        exit 1
    }
    exit $retry
}


# Delete a GitHub release and its tag
def "main delete" [
    tag: string # release tag such as v0.1.0
]: nothing -> nothing {
    ^gh release delete $tag --cleanup-tag --yes
}


# Build, publish, and manage project releases
def main []: nothing -> string {
    help main
}
