{
  description = "Valence coprocessor app";

  nixConfig.extra-experimental-features = "nix-command flakes";
  nixConfig.extra-substituters = "https://coffeetables.cachix.org";
  nixConfig.extra-trusted-public-keys = ''
    coffeetables.cachix.org-1:BCQXDtLGFVo/rTG/J4omlyP/jbtNtsZIKHBMTjAWt8g=
  '';

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-24.11";
    
    flake-parts.url = "github:hercules-ci/flake-parts";
    fp-addons.url = "github:timewave-computer/flake-parts-addons";

    devshell.url = "github:numtide/devshell";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    sp1-nix.url = "github:timewave-computer/sp1.nix";
    crate2nix.url = "github:timewave-computer/crate2nix";
  };

  outputs = inputs@{ self, flake-parts, ... }:
    flake-parts.lib.mkFlake {inherit inputs;} ({moduleWithSystem, ...}: {
      imports = [
        inputs.devshell.flakeModule
        inputs.crate2nix.flakeModule
        inputs.fp-addons.flakeModules.tools
      ];

      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];

      perSystem = {
        lib,
        config,
        inputs',
        pkgs,
        system,
        ...
      }: {
        _module.args.pkgs = import inputs.nixpkgs {
          inherit system;
          overlays = [ inputs.rust-overlay.overlays.default ];
        };

        crate2nix = {
          cargoNix = ./Cargo.nix;
          devshell.name = "default";
          toolchain = {
            rust = pkgs.rust-bin.nightly.latest.default;
            cargo = pkgs.rust-bin.nightly.latest.default;
          };
          crateOverrides = inputs'.sp1-nix.tools.crateOverrides // {
            valence-coprocessor-ethereum-service = attrs:
              let
                src = lib.cleanSourceWith {
                  filter = config.crate2nix.cargoNix.internal.sourceFilter;
                  src = ./.;
                };
              in
              {
                inherit src;
                sourceRoot = "${src.name}/crates/lightclient/service";
                meta.mainProgram = attrs.crateName;
              };
          };
        };

        packages = {
          service = config.crate2nix.packages.valence-coprocessor-ethereum-service;
        };

        checks = {
          service = config.crate2nix.checks.valence-coprocessor-ethereum-service;
        };

        devshells.default = {
          packages = with pkgs; [
            curl
            jq
            clang
            taplo
            toml-cli
            lld
          ];
          
          env = [
            {
              name = "OPENSSL_DIR";
              value = "${pkgs.lib.getDev pkgs.openssl}";
            }
            {
              name = "OPENSSL_LIB_DIR";
              value = "${pkgs.lib.getLib pkgs.openssl}/lib";
            }
            {
              name = "LIBCLANG_PATH";
              value = pkgs.lib.makeLibraryPath [ pkgs.libclang ];
            }
          ];
          
        };
      };

      flake.systemModules.service = moduleWithSystem (
        { self', ... }:
        { lib, ...}:
        {
          systemd.services = {
            valence-coprocessor-ethereum = {
              enable = true;
              serviceConfig = {
                Type = "simple";
                DynamicUser = true;
                StateDirectory = "valence-coprocessor-ethereum";
                ExecStart = lib.getExe self'.packages.service;
              };
              wantedBy = [ "system-manager.target" ];
            };
          };
        }
      );
    });
}
