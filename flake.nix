{
  nixConfig = {
    extra-substituters = [
      "https://cache.nixos.org/"
      "https://mmwave.cachix.org"
      "https://nix-community.cachix.org"
    ];
    extra-experimental-features = "nix-command flakes";
    extra-trusted-public-keys = [
      "mmwave.cachix.org-1:51WVqkk3jgt8S5rmsTZVsFvPw06FpTd1niyrFzJ6ucQ="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    cargo2nix.url = "github:cargo2nix/cargo2nix/release-0.11.0";
    nix-formatter-pack.url = "github:Gerschtli/nix-formatter-pack";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = inputs: with inputs;
  flake-utils.lib.eachDefaultSystem (system: 
    let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ cargo2nix.overlays.default ];
      };
      rustPkgs = pkgs.rustBuilder.makePackageSet {
        rustVersion = "1.75.0";
        extraRustComponents = ["clippy"];
        packageFun = import ./Cargo.nix;
      };
      mmwave = {
        discovery = (rustPkgs.workspace.mmwave-discovery {});
        machine = (rustPkgs.workspace.mmwave-machine {});
        dashboard  = (rustPkgs.workspace.mmwave-dashboard {});
      };
    in {
      formatter = inputs.nix-formatter-pack.lib.mkFormatter {
        pkgs = nixpkgs.legacyPackages.${system};
        config.tools = {
          alejandra.enable = false;
          deadnix.enable = true;
          nixpkgs-fmt.enable = true;
          statix.enable = true;
        };
      };

      packages = mmwave;

      devShells.default = let
        libs = with pkgs; [
          libGL
          libxkbcommon
          udev
          openssl
          wayland
          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
          dbus
          pkg-config
        ];
      in (rustPkgs.workspaceShell {
        RUST_SRC_PATH = pkgs.pkgsBuildHost.rustPlatform.rustLibSrc;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

        packages = with pkgs.pkgsBuildHost; libs ++ [
          rust-analyzer
          natscli
          nats-server
          cargo2nix.packages.${system}.cargo2nix
        ];
      });
    }
  );
}
