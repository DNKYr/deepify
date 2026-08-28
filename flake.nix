{
  description = "Deepify local focus prototype";

  # Immutable nixos-26.05 snapshot served by FlakeHub's public cache.
  inputs.nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.2605.1012700.tar.gz";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in {
      packages = eachSystem (pkgs:
        let
          linuxLibraries = with pkgs; [
            alsa-lib
            dbus
            glib
            gtk3
            libayatana-appindicator
            openssl
            webkitgtk_4_1
          ];
        in {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "deepify";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            npmDeps = pkgs.fetchNpmDeps {
              src = self;
              hash = "sha256-dG6ZMDpKMc7MMsNSpncJmCoF1L3SboP1j7ZJhV2KE4Y=";
            };
            nativeBuildInputs = with pkgs; [
              clippy
              nodejs_24
              npmHooks.npmConfigHook
              pkg-config
              rustfmt
            ];
            buildInputs = linuxLibraries;
            preBuild = "npm run build";
            cargoBuildFlags = [
              "--workspace"
              "--features"
              "deepify-desktop/custom-protocol"
            ];
            cargoTestFlags = [
              "--workspace"
              "--features"
              "deepify-desktop/custom-protocol"
            ];
            preCheck = ''
              npm test
              npm run typecheck
              npm run lint
              npm run format:check
              cargo fmt --all -- --check
              cargo clippy --workspace --all-targets --features deepify-desktop/custom-protocol -- -D warnings
            '';
          };
        });
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            alsa-lib
            cargo
            clippy
            dbus
            ffmpeg-headless.bin
            glib
            gtk3
            libayatana-appindicator
            nodejs_24
            openssl
            pkg-config
            rustc
            rustfmt
            sqlite
            webkitgtk_4_1
          ];
          shellHook = ''
            echo "Deepify development shell (Phase 2 simulated integrations)"
          '';
        };
      });
      checks = eachSystem (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) default;
      });
    };
}
