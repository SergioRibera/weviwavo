{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      baseSystem:
      let
        cargoManifest = fromTOML (builtins.readFile ./Cargo.toml);

        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          system = baseSystem;
          inherit overlays;
          config.allowUnfree = true;
        };

        libraries = with pkgs; [
          libGL
          fontconfig
          freetype
          pkgs.stdenv.cc.cc.lib
          rustPlatform.bindgenHook
          libx11
          libxcb
          libxkbcommon
          openssl

          wayland

          vulkan-loader

          alsa-lib
        ];

        appPkg = (pkgs.rustPlatform.buildRustPackage.override { stdenv = pkgs.clangStdenv; }) (finalAttrs: {
          pname = cargoManifest.package.name;
          version = cargoManifest.package.version;
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = false;
          cargoExtraArgs = "-F desktop";

          nativeBuildInputs =
            with pkgs;
            [
              pkg-config
              python3
              makeWrapper
              removeReferencesTo

              rustPlatform.bindgenHook
              autoPatchelfHook
            ]
            ++ lib.optionals stdenv.buildPlatform.isDarwin [
              libiconv
              cctools.libtool
            ];
          runtimeDependencies =
            with pkgs;
            [ noto-fonts-color-emoji ]
            ++ lib.optionals stdenv.isLinux [
              wayland
              libxkbcommon
            ];

          makeWrapperArgs = [
            "--prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath libraries}"
          ];
          buildInputs = libraries;

          postFixup = ''
            remove-references-to -t "$SKIA_SOURCE_DIR" $out/bin/${cargoManifest.package.name}
            patchelf --set-rpath "${pkgs.lib.makeLibraryPath libraries}" $out/bin/${cargoManifest.package.name}
          '';
          disallowedReferences = [ finalAttrs.SKIA_SOURCE_DIR ];

          SKIA_NINJA_COMMAND = "${pkgs.ninja}/bin/ninja";
          SKIA_GN_COMMAND = "${pkgs.gn}/bin/gn";
          SKIA_ENABLE_TOOLS = "false";
          SKIA_LIBRARY_DIR = "${pkgs.skia}/lib";
          SKIA_SOURCE_DIR =
            let
              repo = pkgs.fetchFromGitHub {
                owner = "rust-skia";
                repo = "skia";
                # see rust-skia:skia-bindings/Cargo.toml#package.metadata skia
                tag = "m143-0.90.0";
                hash = "sha256-wDbQ6JkV3Kahz/WsOTE6mLpI4cPfKKy8a3IpQ3b1uDY=";
              };
              # The externals for skia are taken from skia/DEPS
              externals = pkgs.linkFarm "skia-externals" (
                pkgs.lib.mapAttrsToList (name: value: {
                  inherit name;
                  path = pkgs.fetchgit value;
                }) (pkgs.lib.importJSON ./skia-externals.json)
              );
            in
            pkgs.runCommand "source" { } ''
              cp -R ${repo} $out
              chmod -R +w $out
              ln -s ${externals} $out/third_party/externals
            '';
        });
      in
      {
        apps.default = {
          type = "app";
          program = "${appPkg}/bin/${cargoManifest.package.name}";
        };
        packages.default = appPkg;
        devShells.default = pkgs.mkShell {
          packages =
            with pkgs;
            [
              rust-bin.stable.latest.default

              cargo-dist
              cargo-release
              dioxus-cli
              git-cliff

              pkg-config
              wayland
              python3
            ]
            ++ libraries;
          buildInputs = with pkgs; [
            llvmPackages.bintools
          ];
          LD_LIBRARY_PATH = "${pkgs.lib.makeLibraryPath libraries}";
          # jemalloc configure: needs _GNU_SOURCE for strerror_r detection.
          # Disable fortify hardening — Nix GCC wrapper injects -D_FORTIFY_SOURCE=2
          # but jemalloc configure uses -O0, causing a -Werror=cpp failure.
          shellHook = ''
            export CFLAGS="-D_GNU_SOURCE ''${CFLAGS:-}"
            export NIX_HARDENING_ENABLE="''${NIX_HARDENING_ENABLE//fortify/}"
          '';
        };
      }
    );
}
