# common.nix
{ pkgs, self, nodeHostName, mmwave, inputs, ... }:
let
  user = "mmwave";
  password = "mmwave";
  overlay = _final: super: {
    makeModulesClosure = x:
      super.makeModulesClosure (x // { allowMissing = true; });
  };
in
{

  imports = [
    inputs.home-manager.nixosModules.home-manager
    {
      home-manager.useGlobalPkgs = true;
      home-manager.useUserPackages = true;

      home-manager.users.${user} = _: {
        home.stateVersion = "24.05";
      };
    }
  ];

  nixpkgs.overlays = [ overlay ];

  environment.systemPackages = with pkgs; [
    btop
    helix
    git
    mmwave.mmwave-dashboard
    mmwave.mmwave-discovery
    mmwave.mmwave-machine
  ];

  users = {
    mutableUsers = false;
    users."${user}" = {
      isNormalUser = true;
      inherit password;
      extraGroups = [ "wheel" ];
    };
    users.root = {
      inherit password;
    };
  };

  networking = {
    useDHCP = true;
    hostName = nodeHostName;
    wireless = {
      enable = true;
      networks = {
        ammwbase = {
          psk = "mmwave";
        };
      };
    };
  };

  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = true;
      KbdInteractiveAuthentication = true;
      PermitRootLogin = "yes";
      X11Forwarding = true;
    };
  };
  programs.ssh.startAgent = true;
  networking.firewall.allowedTCPPorts = [ 22 ];

  hardware.enableRedistributableFirmware = true;

  # Enable automatic updates
  # https://nixos.wiki/wiki/Automatic_system_upgrades
  system.autoUpgrade = {
    enable = true;
    allowReboot = true;
    flake = "path:${self.outPath}#${nodeHostName}"; # flake path must be preceded with "path:" because otherwise nix build will get confused when we ask to build a flake in the nix store
    flags = [
      "--update-input"
      "nixpkgs"
      "-L" # print build logs
    ];
  };

  environment.etc.nixos = {
    source = "${self.outPath}";
  };

  environment.variables = {
    FLAKE_PATH = "path:${self.outPath}#${nodeHostName}";
  };

  system.stateVersion = "24.05";
}
