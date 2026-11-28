set working-directory := '..'

check:
    nix develop ./.tools --command check

fix:
    nix develop ./.tools --command fix
