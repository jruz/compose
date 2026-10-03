wait_for_startup() {
    COMPOSE_DEADLINE=$(( $(date +%s) + COMPOSE_STARTUP_TIMEOUT ))
    until eval "$1"; do
        if ! kill -0 "$COMPOSE_PID" 2>/dev/null; then
            echo "compose exited before readiness" >&2
            return 1
        fi
        if test "$(date +%s)" -ge "$COMPOSE_DEADLINE"; then
            echo "compose startup timed out after ${COMPOSE_STARTUP_TIMEOUT}s" >&2
            return 124
        fi
        sleep 0.01
    done
}

wait_for_cleanup() {
    COMPOSE_CLEANUP_DEADLINE=$(( $(date +%s) + 15 ))
    while kill -0 "$COMPOSE_PID" 2>/dev/null; do
        if test "$(date +%s)" -ge "$COMPOSE_CLEANUP_DEADLINE"; then
            echo "compose cleanup timed out; sending SIGKILL" >&2
            kill -KILL "$COMPOSE_PID"
            return 124
        fi
        sleep 0.01
    done
}

cleanup_interrupt() {
    COMPOSE_STATUS=$?
    trap - EXIT TERM
    if kill -0 "$COMPOSE_PID" 2>/dev/null; then
        kill -TERM "$COMPOSE_PID"
        if wait_for_cleanup; then
            echo "compose cleanup completed" >&2
        else
            echo "compose cleanup exceeded its deadline" >&2
        fi
        if wait "$COMPOSE_PID"; then
            echo "compose cleanup exited successfully" >&2
        else
            echo "compose cleanup exited with status $?" >&2
        fi
    fi
    cat "$COMPOSE_CAPTURE" || exit $?
    rm -r "$COMPOSE_TEST_DIR" || exit $?
    exit "$COMPOSE_STATUS"
}

COMPOSE_TEST_DIR=$(mktemp -d) || exit $?
COMPOSE_CAPTURE="$COMPOSE_TEST_DIR/output"
