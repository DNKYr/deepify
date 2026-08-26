{
  description = "Deepify local focus prototype";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in {
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [ rustc cargo rustfmt clippy nodejs_24 npm pkg-config sqlite ];
          shellHook = ''
            echo "Deepify development shell (Phase 2 simulated integrations)"
          '';
        };
      });
      checks = eachSystem (pkgs: {
        rust = pkgs.stdenv.mkDerivation {
          pname = "deepify-check"; version = "0.1.0"; src = self;
          nativeBuildInputs = [ pkgs.rustc pkgs.cargo ];
          buildPhase = "cargo test --workspace";
          installPhase = "touch $out";
        };
      });
    };
}
