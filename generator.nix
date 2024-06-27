{ nixpkgs, inputs, self, mmwave, forAllSystems, ... }:
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
  buildGenerator = node: mmwave:
    inputs.nixos-generators.nixosGenerate {
      inherit (node) system;
      inherit (node) format;
      inherit (node) modules;
      specialArgs = {
        inherit nixpkgs;
        inherit self;
        nodeHostName = node.name;
        mmwave = mmwave.${node.system};
        inherit inputs;
      };
    };
  buildConfiguration = node: mmwave:
    let
      generated = buildGenerator node mmwave;
    in
    nixpkgs.lib.nixosSystem {
      inherit (generated) system;
      modules = node.modules ++ [ ./formats/${generated.format}.nix ];
      inherit (generated) specialArgs;
    };
in
{
  generators = forAllSystems (system: builtins.listToAttrs (
    map
      (node: { inherit (node) name; value = buildGenerator node mmwave.${system}; })
      nodes
  ));

  nixosConfigurations = forAllSystems (system: builtins.listToAttrs (
    map
      (node: { inherit (node) name; value = buildConfiguration node mmwave.${system}; })
      nodes
  ));
}
