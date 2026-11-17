set working-directory := '..'

fmt-check:
    oxfmt --check .

fmt-fix:
    oxfmt --write .
