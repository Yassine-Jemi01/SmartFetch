{
  description = "SmartFetch - a fast terminal system fetch with animated 3D views";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        # A committed Cargo.lock is required for a reproducible Nix package.
        if builtins.pathExists ./Cargo.lock then
          let
            smartfetch = pkgs.rustPlatform.buildRustPackage {
              pname = "smartfetch";
              version = "1.1.0";
              src = self;
              cargoLock.lockFile = ./Cargo.lock;

              doCheck = true;
              cargoTestFlags = [ "--all-targets" ];

              meta = {
                description = "Fast system information with animated 3D terminal rendering";
                homepage = "https://github.com/Yassine-Jemi01/SmartFetch";
                license = pkgs.lib.licenses.mit;
                mainProgram = "sfetch";
                platforms = systems;
              };
            };
          in
          {
            default = smartfetch;
            sfetch = smartfetch;
          }
        else
          { }
      );

      devShells = forAllSystems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              gnumake
            ];

            shellHook = ''
              echo "SmartFetch development environment"
              echo "Build: cargo build --release"
              echo "Test:  cargo test --all-targets"
            '';
          };
        }
      );

      overlays.default = final: prev:
        if builtins.pathExists ./Cargo.lock then {
          smartfetch = final.rustPlatform.buildRustPackage {
            pname = "smartfetch";
            version = "1.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
            cargoTestFlags = [ "--all-targets" ];
          };
        } else { };
    };
}
