{
  description = "A program for managing systemd services through a TUI (Terminal User Interfaces).";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
    ];

    forAllSystems = function: nixpkgs.lib.genAttrs systems (system: function nixpkgs.legacyPackages.${system});
  in {
    packages = forAllSystems (pkgs: rec {
      default = systemd-manager-tui;
      systemd-manager-tui = let
        p = (pkgs.lib.importTOML ./Cargo.toml).package;
        rev = self.dirtyRev or self.rev;
      in
        pkgs.rustPlatform.buildRustPackage {
          pname = p.name;
          inherit (p) version;

          src = pkgs.lib.fileset.toSource {
            root = ./.;
            fileset = pkgs.lib.fileset.intersection (pkgs.lib.fileset.fromSource (pkgs.lib.sources.cleanSource ./.)) (
              pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./src
              ]
            );
          };

          cargoLock.lockFile = ./Cargo.lock;

          buildInputs = [pkgs.openssl];
          nativeBuildInputs = [pkgs.pkg-config];

          env = {
            BUILD_REV = rev;
          };

          meta = {
            inherit (p) description homepage;
            license = pkgs.lib.licenses.mit;
            maintainers = with pkgs.lib.maintainers; [tuxdotrs];
            mainProgram = "systemd-manager-tui";
          };
        };
    });
  };
}
