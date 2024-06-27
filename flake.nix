{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };
  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, ... }:
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
      crateExpression =
        { openssl
        , libiconv
        , lib
        , pkg-config
        , qemu
        , stdenv
        , udev
        , dbus
        , wayland
        , xorg
        , libGL
        , libxkbcommon
        , glibc
        }:
        let 
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
          nativeBuildInputs = [
            pkg-config
            stdenv.cc
          ] ++ lib.optionals stdenv.buildPlatform.isDarwin [
            libiconv
          ];
          depsBuildBuild = [
            qemu
          ];
          buildInputs = [
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
          ];
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER = "${stdenv.cc.targetPrefix}cc";
          CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUNNER = "qemu-aarch64";
          cargoExtraArgs = "--target aarch64-unknown-linux-gnu";
          CARGO_BUILD_TARGET = "aarch64-unknown-linux-gnu";
          rustflags = "-c target-feature=+crt-static";

          HOST_CC = "${stdenv.cc.nativePrefix}cc";
          TARGET_CC = "${stdenv.cc.targetPrefix}cc";
        };
        in
        {
          mmwave-discovery = craneLib.buildPackage commonArgs // {
            pname = "mmwave-discovery";
            cargoExtraArgs = "-p mmwave-discovery";
          };
          mmwave-machine = craneLib.buildPackage commonArgs // {
            pname = "mmwave-machine";
            cargoExtraArgs = "-p mmwave-machine";
          };
          mmwave-dashboard = craneLib.buildPackage commonArgs // { 
            pname = "mmwave-dashboard";
            cargoExtraArgs = "-p mmwave-dashboard";
          };
        };

      mmwave = pkgs.callPackage crateExpression { };

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
