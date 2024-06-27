{ craneLib
, openssl
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
}

