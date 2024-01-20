{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, utils, naersk }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        naersk-lib = pkgs.callPackage naersk { };
      in
      {
        defaultPackage = naersk-lib.buildPackage ./.;
        devShell = with pkgs; mkShell {
          nativeBuildInputs = with pkgs; [ pkg-config udev alsa-lib pkg-config ];
          buildInputs = with pkgs; [ 
            cargo rustc rustfmt rust-analyzer rustPackages.clippy rustup
            openssl
            # X support:
            xorg.libX11 
            xorg.libXcursor 
            xorg.libXi 
            xorg.libXrandr
          ]; 
          RUST_SRC_PATH = rustPlatform.rustLibSrc;
          shellHook = ''export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath [
            pkgs.vulkan-loader
            # Wayland Support
            pkgs.wayland
            pkgs.libxkbcommon
          ]}" && export PATH="$PATH:$HOME/.cargo/bin"
          '';
         };
      });
}

