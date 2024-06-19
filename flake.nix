{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.lib.${system};
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
        libPath = pkgs.lib.makeLibraryPath libs;
        my-crate = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);
          doCheck = true;
          name = "mmwave-dash";
          nativeBuildInputs = [ pkgs.makeWrapper ];
          buildInputs = with pkgs; [
            xorg.libxcb
          ];
          postInstall = ''
            wrapProgram "$out/bin/mmwave-dash" --prefix LD_LIBRARY_PATH : "${libPath}"
          '';
        };
      in
      {
        checks = {
          inherit my-crate;
        };

        packages.default = my-crate;

        app.default = flake-utils.lib.mkApp {
          drv = my-crate;
        };

        devShell = craneLib.devShell {
          checks = self.checks.${system};

          RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          LD_LIBRARY_PATH = libPath;

          packages = with pkgs; [
            rustfmt
            rust-analyzer
            rustPackages.clippy
            rustup
            natscli
            nats-server
          ] ++ libs;
        };
      });
}
