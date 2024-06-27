{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    nixos-generators = {
      url = "github:nix-community/nixos-generators";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, nixos-generators, ... }:
  flake-utils.lib.eachSystem ["x86_64-linux"] (localSystem:
    let
      crossSystem = "aarch64-linux";
      pkgs = import nixpkgs {
        inherit localSystem crossSystem;
        overlays = [ (import rust-overlay) ];
      };
      rustToolchain = pkgs.rust-bin.stable.latest.default.override {
        targets = [ "aarch64-unknown-linux-gnu" ];
      };
      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
      crateExpression = import ./package.nix;
      mmwave = pkgs.callPackage crateExpression { inherit craneLib; };
      pi4 = nixos-generators.nixosGenerate {
        specialArgs = {
          inherit nixpkgs;
        };
        system = "aarch64-linux";
        format = "sd-aarch64";
        modules = [
          ./pi4.nix
        ];
      };
      libs = with pkgs.pkgsBuildHost; [
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
    {
      checks = {
        inherit mmwave;
      };

      packages = mmwave;

      generators = {
        inherit pi4;
      };

      apps.mmwave = flake-utils.lib.mkApp {
        drv = pkgs.writeScriptBin "mmwave-discovery" ''
          ${pkgs.pkgsBuildBuild.qemu}/bin/qemu-aarch64 ${mmwave}/bin/mmwave-discovery
        # '';
      };

      devShells.default = craneLib.devShell {
        checks = self.checks.${localSystem};

        RUST_SRC_PATH = pkgs.pkgsBuildHost.rustPlatform.rustLibSrc;
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;

        packages = with pkgs.pkgsBuildHost; libs ++ [
          rust-analyzer
          natscli
          nats-server
        ];
      };
    }
  );
}
