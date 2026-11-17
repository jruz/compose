{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-26.05";
    nixpkgs-unstable.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      nixpkgs-unstable,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        unstable = import nixpkgs-unstable { inherit system; };
      in
      {
        devShells.default = pkgs.mkShell {
          packages =
            (with pkgs; [
              deadnix
              fd
              just
              nixfmt
              nu-lint
              nushell
              renovate
              statix
              unstable.woodpecker-cli
            ])
            ++
              map
                (
                  name:
                  pkgs.writeShellApplication {
                    inherit name;
                    runtimeInputs = [ pkgs.just ];
                    text = ''
                      exec just --justfile .tools/justfile ${name} "$@"
                    '';
                  }
                )
                [
                  "check"
                  "fix"
                ];
        };
      }
    );
}
