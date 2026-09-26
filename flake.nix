{
  description = "Deepify local focus desktop";

  # Immutable nixos-26.05 snapshot served by FlakeHub's public cache.
  inputs.nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.2605.1012700.tar.gz";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f (import nixpkgs { inherit system; }));
    in
    {
      packages = eachSystem (pkgs:
        let
          linuxLibraries = with pkgs; [
            alsa-lib
            dbus
            glib
            gsettings-desktop-schemas
            gtk3
            libayatana-appindicator
            openssl
            webkitgtk_4_1
          ];
          basePackage = pkgs.rustPlatform.buildRustPackage {
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
              desktop-file-utils
              nodejs_24
              npmHooks.npmConfigHook
              pkg-config
              rustfmt
              wrapGAppsHook3
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
            postInstall = ''
              install -Dm644 apps/desktop/src-tauri/icons/icon.svg \
                "$out/share/icons/hicolor/scalable/apps/com.deepify.desktop.svg"
              mkdir -p "$out/share/applications"
              cat > "$out/share/applications/com.deepify.desktop.desktop" <<EOF
              [Desktop Entry]
              Type=Application
              Name=Deepify
              Comment=Local focus sessions with application and website restrictions
              Exec=$out/bin/deepify
              Icon=com.deepify.desktop
              Terminal=false
              Categories=Office;Utility;
              StartupWMClass=com.deepify.desktop
              EOF
              desktop-file-validate "$out/share/applications/com.deepify.desktop.desktop"
            '';
            preCheck = ''
              npm test
              npm run typecheck
              npm run lint
              npm run format:check
              cargo fmt --all -- --check
              cargo clippy --workspace --all-targets --features deepify-desktop/custom-protocol -- -D warnings
            '';
          };
          nativeManifest = pkgs.runCommand "deepify-native-host-manifest-0.1.0" { } ''
            install -Dm644 ${./extensions/firefox/native-manifest.json} \
              $out/lib/mozilla/native-messaging-hosts/com.deepify.browser.json
            substituteInPlace \
              $out/lib/mozilla/native-messaging-hosts/com.deepify.browser.json \
              --replace-fail '"path": "deepify-browser-native-host"' \
                '"path": "${basePackage}/bin/deepify-browser-native-host"'
          '';
          browserNativeHost = pkgs.runCommand "deepify-browser-native-host-0.1.0" { } ''
            mkdir -p $out/bin $out/lib/mozilla/native-messaging-hosts
            ln -s ${basePackage}/bin/deepify-browser-native-host \
              $out/bin/deepify-browser-native-host
            ln -s ${nativeManifest}/lib/mozilla/native-messaging-hosts/com.deepify.browser.json \
              $out/lib/mozilla/native-messaging-hosts/com.deepify.browser.json
          '';
          firefoxExtension = pkgs.runCommand "deepify-firefox-extension-0.1.0" { nativeBuildInputs = [ pkgs.zip pkgs.jq ]; } ''
            mkdir -p $out
            workdir=$(mktemp -d)
            cp ${./extensions/firefox}/{manifest.json,background.js,blocked.html,blocked.js,blocked.css} "$workdir/"
            test "$(jq -r '.browser_specific_settings.gecko.id' "$workdir/manifest.json")" = "focus@deepify.local"
            test "$(jq -r '.permissions[]' "$workdir/manifest.json" | grep -c '^cookies$' || true)" = 0
            ! grep -R -E "Phase 2|Preview only|simulated.*website" "$workdir"
            touch -d @1 "$workdir"/*
            (cd "$workdir" && zip -X -q -r "$out/deepify-focus-companion.xpi" .)
          '';
        in
        {
          default = pkgs.symlinkJoin {
            name = "deepify-0.1.0";
            paths = [ basePackage nativeManifest ];
          };
          browser-native-host = browserNativeHost;
          firefox-extension = firefoxExtension;
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
            gsettings-desktop-schemas
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
            export XDG_DATA_DIRS="${pkgs.lib.removeSuffix "/glib-2.0/schemas" (pkgs.glib.getSchemaPath pkgs.gtk3)}:${pkgs.lib.removeSuffix "/glib-2.0/schemas" (pkgs.glib.getSchemaPath pkgs.gsettings-desktop-schemas)}:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
            echo "Deepify development shell (Phase 5 refinement and release verification)"
          '';
        };
      });
      checks = eachSystem (pkgs: {
        inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) default;
      });
    };
}
