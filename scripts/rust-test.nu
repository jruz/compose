def --wrapped main [
    ...filters: string # Arguments forwarded to cargo nextest
]: nothing -> nothing {
    let root = (^git rev-parse --show-toplevel | str trim)
    let socket_group = (^stat --format %g /var/run/docker.sock)
    let group = ($socket_group | str trim)
    (^sandbox
        --network host
        --env DOCKER_CONFIG=/tmp/docker
        --group-add $group
        --mount type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock
        --mount $"type=bind,src=($root),dst=($root)"
        --workdir $root
        -- cargo nextest run --no-tests=pass ...$filters)
}
