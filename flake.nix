# SPDX-FileCopyrightText: 2022 Noah Fontes
#
# SPDX-License-Identifier: Apache-2.0

{
  inputs = {
    mozilla = {
      url = "github:mozilla/nixpkgs-mozilla";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, mozilla, nixpkgs }: with nixpkgs.lib; let
    metadata = importTOML ./Cargo.toml;

    allSystems = builtins.map (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ mozilla.overlays.rust ];
      };

      rustPkgs = pkgs.rustChannelOf {
        date = "2022-08-21";
        channel = "nightly";
        sha256 = "sha256-+14+dZE3GrVmef6F4Dui7EQBbq73mVkeFzZTXPHjE/M=";
      };
      rustPlatform = pkgs.makeRustPlatform { cargo = rustPkgs.rust; rustc = rustPkgs.rust; };
    in rec {
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = [
          pkgs.lldb
          rustPkgs.rust
        ];
      };

      packages.${system} = rec {
        systemd-user-sleep = rustPlatform.buildRustPackage {
          pname = metadata.package.name;
          version = metadata.package.version;

          src = ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          postInstall = ''
            mkdir -p $out/lib/systemd/user
            cp examples/sleep.target $out/lib/systemd/user/sleep.target
            substitute examples/systemd-user-sleep.service $out/lib/systemd/user/systemd-user-sleep.service \
              --replace 'ExecStart=systemd-user-sleep' "ExecStart=$out/bin/systemd-user-sleep"
          '';

          meta = {
            inherit (metadata.package) description;
            homepage = metadata.package.repository;
            license = licenses.asl20;
          };
        };

        default = systemd-user-sleep;
      };
    }) systems.flakeExposed;
  in (builtins.foldl' recursiveUpdate {} allSystems) // {
    nixosModules = rec {
      systemd-user-sleep = { pkgs, ... }: {
        systemd.packages = [
          self.packages.${pkgs.hostPlatform.system}.systemd-user-sleep
        ];

        # https://github.com/NixOS/nixpkgs/issues/81138
        systemd.user.services."systemd-user-sleep".wantedBy = [ "default.target" ];
      };

      default = systemd-user-sleep;
    };
  };
}
