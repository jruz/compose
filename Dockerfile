FROM docker.io/nixos/nix:2.34.1

RUN echo "experimental-features = nix-command flakes" >> /etc/nix/nix.conf

COPY flake.nix flake.lock rust-toolchain.toml Cargo.toml Cargo.lock /build/
WORKDIR /build

RUN nix develop --command true

WORKDIR /app
CMD ["nix", "develop", "--command", "just", "check"]
