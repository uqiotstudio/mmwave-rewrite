{ nixpkgs, inputs, self, mmwave, ... }:
let
  nodes = [
    {
      name = "pi4";
      system = "aarch64-linux";
      format = "sd-aarch64";
      inherit nixpkgs;
      modules = [
        ./common.nix
        "${nixpkgs}/nixos/modules/installer/sd-card/sd-image-aarch64.nix"
      ];
    }
  ];
  buildGenerator = node:
    inputs.nixos-generators.nixosGenerate {
      inherit (node) system;
      inherit (node) format;
      inherit (node) modules;
      specialArgs = {
        inherit nixpkgs;
        inherit self;
        nodeHostName = node.name;
        mmwave = mmwave;
        inherit inputs;
      };
    };
  buildConfiguration = node:
    let
      generated = buildGenerator node;
    in
    nixpkgs.lib.nixosSystem {
      inherit (generated) system;
      modules = node.modules ++ [ ./formats/${generated.format}.nix ];
      inherit (generated) specialArgs;
    };
in
{
  generators = builtins.listToAttrs (
    map
      (node: { inherit (node) name; value = buildGenerator node; })
      nodes
  );

  nixosConfigurations = builtins.listToAttrs (
    map
      (node: { inherit (node) name; value = buildConfiguration node; })
      nodes
  );
}
