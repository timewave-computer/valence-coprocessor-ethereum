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
          crateOverrides =
            let
              src = lib.cleanSourceWith {
                filter = config.crate2nix.cargoNix.internal.sourceFilter;
                src = ./.;
              };
            in inputs'.sp1-nix.tools.crateOverrides // {
              valence-coprocessor-ethereum-service = attrs: {
                inherit src;
                sourceRoot = "${src.name}/crates/lightclient/service";
                meta.mainProgram = attrs.crateName;
              };
              valence-coprocessor-ethereum-lightclient = attrs: {
                inherit src;
                sourceRoot = "${src.name}/crates/lightclient/lib";
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

      flake.nixosModules.service = moduleWithSystem (
        { self', ... }:
        { lib, config, ...}:
        let
          cfg = config.services.valence-coprocessor.ethereum;
        in
        {
          options = {
            services.valence-coprocessor.ethereum = {
              package = lib.mkOption {
                type = lib.types.package;
                default = self'.packages.service;
              };
              flags = lib.mkOption {
                type = lib.types.listOf (lib.types.str);
                default = [];
              };
            };
          };
          config = {
            systemd.services = {
              valence-coprocessor-ethereum = {
                enable = true;
                restartIfChanged = false;
                serviceConfig = {
                  Type = "simple";
                  DynamicUser = true;
                  StateDirectory = "valence-coprocessor-ethereum";
                  ExecStart = "${lib.getExe cfg.package} ${lib.concatStringsSep " " cfg.flags}";
                };
                wantedBy = [ "multi-user.target" ];
              };
            };
          };
        }
      );
    });
}
