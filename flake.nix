{
  description = "compose: lightweight process orchestrator";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = builtins.fromTOML (builtins.readFile ./rust-toolchain.toml);
        rustVersion = rustToolchain.toolchain.channel;
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        revision = self.rev or "dirty";
        sandboxPath = pkgs.lib.makeBinPath [
          rust
          pkgs.bash
          pkgs.bun
          pkgs.cargo-edit
          pkgs.cargo-nextest
          pkgs.cargo-outdated
          pkgs.cargo-watch
          pkgs.coreutils
          pkgs.curl
          pkgs.docker-client
          pkgs.findutils
          pkgs.gcc
          pkgs.git
          pkgs.gnugrep
          pkgs.just
          pkgs.nushell
          pkgs.podman
          pkgs.ripgrep
          pkgs.woodpecker-cli
        ];
        mkSandbox =
          name: dockerEnabled:
          pkgs.writeShellScriptBin name (
            ''
              SELF_DIR="$(dirname "$(readlink -f "$0")")"
              ARGS=(
                --unshare-all
                --share-net
                --die-with-parent
                --ro-bind /nix /nix
                --ro-bind /etc/resolv.conf /etc/resolv.conf
                --ro-bind /etc/ssl /etc/ssl
                --ro-bind-try /etc/static /etc/static
                --ro-bind-try /etc/nix /etc/nix
                --ro-bind-try /run/current-system/sw /run/current-system/sw
                --symlink ${pkgs.coreutils}/bin/env /usr/bin/env
                --symlink ${pkgs.bash}/bin/bash /bin/sh
                --tmpfs "$HOME"
                --tmpfs /tmp
                --bind "$(pwd)" "$(pwd)"
                --ro-bind "$(git rev-parse --path-format=absolute --git-common-dir)" "$(git rev-parse --path-format=absolute --git-common-dir)"
                --bind-try "$HOME/.bun/install/cache" "$HOME/.bun/install/cache"
                --bind-try "$HOME/.cargo/registry" "$HOME/.cargo/registry"
                --bind-try "$HOME/.cargo/git" "$HOME/.cargo/git"
                --ro-bind-try "$HOME/.gitconfig" "$HOME/.gitconfig"
                --ro-bind-try "$HOME/.ssh/known_hosts" "$HOME/.ssh/known_hosts"
                --setenv PATH "${sandboxPath}:$SELF_DIR"
                --setenv HOME "$HOME"
                --setenv TMPDIR /tmp
                --dev /dev
                --proc /proc
              )
            ''
            + pkgs.lib.optionalString dockerEnabled ''
              ARGS+=(--bind /var/run/docker.sock /var/run/docker.sock)
            ''
            + ''
              exec ${pkgs.bubblewrap}/bin/bwrap "''${ARGS[@]}" -- "$@"
            ''
          );
        sandbox = mkSandbox "sandbox" false;
        sandboxDocker = mkSandbox "sandbox-docker" true;
        defaultPackages = with pkgs; [
          deadnix
          fd
          just
          nixfmt
          nushell
          sandbox
          sandboxDocker
          statix
        ];
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          inherit (cargoToml.package) version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          REVISION = revision;
          CARGO_HOME = "$TMPDIR";
          doCheck = false;
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        };

        devShells.default = pkgs.mkShell {
          buildInputs = defaultPackages;

          shellHook = ''
            echo ""
            echo "  Compose Dev Shell"
            echo "  ───────────────────────────"
            echo "  rust         ${rustVersion} (sandbox)"
            echo "  bun          ${pkgs.bun.version} (sandbox)"
            echo ""
          '';
        };

      }
    );
}
