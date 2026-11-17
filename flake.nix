{
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
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
        cargoToml = fromTOML (builtins.readFile ./Cargo.toml);
        rustVersion = (fromTOML (builtins.readFile ./rust-toolchain.toml)).toolchain.channel;
        projectName = cargoToml.package.name;
        revision = self.rev or "dirty";
        release = builtins.fromJSON (builtins.readFile ./release.json);
        platform = release.platforms.${system};
        sandbox = pkgs.writeShellScriptBin "sandbox" ''
          export PATH=${
            pkgs.lib.makeBinPath [
              pkgs.docker-client
              pkgs.git
              pkgs.coreutils
            ]
          }:"$PATH"
          exec ${pkgs.nushell}/bin/nu ${./scripts/sandbox.nu} "$@"
        '';
      in
      {
        packages.default = pkgs.stdenvNoCC.mkDerivation {
          pname = projectName;
          inherit (release) version;
          src = pkgs.fetchurl {
            url = "https://github.com/jruz/${projectName}/releases/download/v${release.version}/${platform.asset}";
            inherit (platform) hash;
          };
          dontUnpack = true;
          dontStrip = true;
          installPhase = ''
            runHook preInstall
            install -Dm755 "$src" "$out/bin/${projectName}"
            install -Dm644 ${./LICENSE} "$out/share/licenses/${projectName}/LICENSE"
            runHook postInstall
          '';
          meta = {
            mainProgram = projectName;
            license = pkgs.lib.licenses.eupl12;
            platforms = builtins.attrNames release.platforms;
          };
        };

        packages.source = rustPlatform.buildRustPackage {
          pname = projectName;
          inherit (cargoToml.package) version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          postInstall = ''
            install -Dm644 ${./LICENSE} "$out/share/licenses/${projectName}/LICENSE"
          '';
          meta = {
            mainProgram = projectName;
            license = pkgs.lib.licenses.eupl12;
          };
          REVISION = revision;
          CARGO_HOME = "$TMPDIR";
          doCheck = false;
          nativeBuildInputs = pkgs.lib.optionals pkgs.stdenv.isLinux [ pkgs.mold ];
          buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            fd
            gh
            git
            just
            nushell
            oxfmt
            podman
            ripgrep
            sandbox
          ];

          shellHook = ''
            echo ""
            echo "  Development Shell"
            echo "  ─────────────────"
            echo "  nixpkgs      ${pkgs.lib.trivial.release}"
            echo "  rust         ${rustVersion} (docker)"
            echo ""
          '';
        };
      }
    );
}
