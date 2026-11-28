set working-directory := '..'

fmt-check:
    fd \
        --hidden \
        --exclude tests/fixtures \
        --type file \
        '^(justfile|.+\.just)$' \
        --exec just \
        --justfile {} \
        --unstable \
        --fmt \
        --check

fmt-fix:
    fd \
        --hidden \
        --exclude tests/fixtures \
        --type file \
        '^(justfile|.+\.just)$' \
        --exec just \
        --justfile {} \
        --unstable \
        --fmt
