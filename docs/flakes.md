### Add this in your nixos config

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    systemd-manager-tui.url = "github:matheus-git/systemd-manager-tui";
  };

  outputs = { self, nixpkgs, systemd-manager-tui, ... }: {
    nixosConfigurations.yourhostname = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        {
          environment.systemPackages = [
            systemd-manager-tui.packages.x86_64-linux.default
          ];
        }
      ];
    };
  };
}
```
