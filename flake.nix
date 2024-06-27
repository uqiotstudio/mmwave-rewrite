{
  nixConfig = {
    extra-substituters = [
      "https://cache.nixos.org/"
      "https://mmwave.cachix.org"
    ];
    extra-experimental-features = "nix-command flakes";
    extra-trusted-public-keys = [
      "mmwave.cachix.org-1:51WVqkk3jgt8S5rmsTZVsFvPw06FpTd1niyrFzJ6ucQ="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
    nixos-hardware.url = "github:NixOS/nixos-hardware/master";
    nixos-generators = {
      url = "github:nix-community/nixos-generators";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-formatter-pack.url = "github:Gerschtli/nix-formatter-pack";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, crane, rust-overlay, ... }@inputs:
    let
      forAllSystems = inputs.nixpkgs.lib.genAttrs [
        "aarch64-linux"
        "x86_64-linux"
      ];
      craneLib = forAllSystems (localSystem: forAllSystems (crossSystem:
        let
          pkgs = import nixpkgs {
            inherit localSystem crossSystem;
            overlays = [ (import rust-overlay) ];
          };
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            targets = [ "aarch64-unknown-linux-gnu" ];
          };
        in
        (crane.mkLib pkgs).overrideToolchain rustToolchain
      ));
      mmwave = forAllSystems (localSystem: forAllSystems (crossSystem:
        let
          pkgs = import nixpkgs {
            inherit localSystem crossSystem;
            overlays = [ (import rust-overlay) ];
          };
        in
        pkgs.callPackage (import ./package.nix) { craneLib = craneLib.${localSystem}.${crossSystem}; inherit crossSystem; }
      ));
      generators = (import ./generator.nix { 
        inherit self nixpkgs inputs mmwave forAllSystems;
      });
    in {
      formatter = forAllSystems (system:
        inputs.nix-formatter-pack.lib.mkFormatter {
          pkgs = nixpkgs.legacyPackages.${system};
          config.tools = {
            alejandra.enable = false;
            deadnix.enable = true;
            nixpkgs-fmt.enable = true;
            statix.enable = true;
          };
        }
      );

      generators = generators;

      packages = mmwave;

      devShell = forAllSystems (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };
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
        in
        craneLib.${system}.${system}.devShell {
          RUST_SRC_PATH = pkgs.pkgsBuildHost.rustPlatform.rustLibSrc;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

          packages = with pkgs.pkgsBuildHost; libs ++ [
            rust-analyzer
            natscli
            nats-server
          ];
        }
      );
    };
}
