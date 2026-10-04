{
  description = "compose: lightweight process orchestrator";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-25.11";
    rust-overlay.url = "github:oxalica/rust-overlay";
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
        sandbox = pkgs.writeShellScriptBin "sandbox" ''
          SELF_DIR="$(dirname "$(readlink -f "$0")")"
          if [ "$#" -eq 0 ]; then
            set -- "''${SHELL:?SHELL must be set when sandbox runs without a command}"
          fi
          exec ${pkgs.bubblewrap}/bin/bwrap \
            --unshare-all \
            --share-net \
            --die-with-parent \
            --ro-bind /nix /nix \
            --ro-bind /etc/resolv.conf /etc/resolv.conf \
            --ro-bind /etc/ssl /etc/ssl \
            --ro-bind-try /etc/static /etc/static \
            --ro-bind-try /etc/nix /etc/nix \
            --ro-bind-try /run/current-system/sw /run/current-system/sw \
            --symlink ${pkgs.coreutils}/bin/env /usr/bin/env \
            --symlink ${pkgs.bash}/bin/bash /bin/sh \
            --tmpfs "$HOME" \
            --bind "$(pwd)" "$(pwd)" \
            --bind /tmp /tmp \
            --bind-try "$HOME/.bun" "$HOME/.bun" \
            --bind-try "$HOME/.cargo" "$HOME/.cargo" \
            --bind-try /var/run/docker.sock /var/run/docker.sock \
            --ro-bind-try "$HOME/.gitconfig" "$HOME/.gitconfig" \
            --ro-bind-try "$HOME/.ssh/known_hosts" "$HOME/.ssh/known_hosts" \
            --setenv PATH "${sandboxPath}:$SELF_DIR" \
            --setenv HOME "$HOME" \
            --setenv REVISION "''${REVISION:-dev}" \
            --dev /dev \
            --proc /proc \
            -- "$@"
        '';
        defaultPackages = with pkgs; [
          deadnix
          fd
          just
          nixfmt
          nushell
          sandbox
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
