{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-manifest = {
      url = "https://static.rust-lang.org/dist/2026-10-04/channel-rust-nightly.toml";
      flake = false;
    };

    darkglass-lv2-extensions = {
      url = "github:Darkglass-Electronics/LV2-Extensions/f25a09ed6bc635fd4cbf75a34c9a735879015ae5";
      flake = false;
    };

    kxstudio-lv2-extensions = {
      url = "github:KXStudio/LV2-Extensions/08b09df09299a8be6912ec68eaaeff19359e00e0";
      flake = false;
    };

    plugin-builder = {
      url = "github:Darkglass-Electronics/plugin-builder/06ef008ccb9276fc3dc110d1ab04d68d3156b3c4";
      flake = false;
    };
  };

  outputs =
    {
      nixpkgs,
      fenix,
      rust-manifest,
      ...
    }@inputs:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];

      shells =
        extend: system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          inherit (pkgs) lib;
          inherit (pkgs.stdenv.hostPlatform) isLinux;
          extra = extend pkgs;

          anagramPkgs =
            if system == "x86_64-linux" then
              pkgs.pkgsCross.aarch64-multiplatform
            else
              nixpkgs.legacyPackages.aarch64-linux;

          crossGcc15 =
            pkgs.pkgsCross.aarch64-multiplatform.buildPackages.gccWithoutTargetLibc.override
              (old: {
                cc = old.cc.override { majorMinorVersion = "15"; };
              });

          nickel =
            if isLinux then
              anagramPkgs.buildPackages.callPackage ./nix/nickel.nix { inherit inputs; }
            else
              pkgs.pkgsCross.aarch64-multiplatform.buildPackages.callPackage ./nix/nickel.nix {
                inherit inputs;
                gcc15 = crossGcc15;
              };

          rust = fenix.packages.${system};

          toolchain = rust.combine [
            ((rust.fromManifestFile rust-manifest).withComponents [
              "cargo"
              "clippy"
              "miri"
              "rust-analyzer"
              "rust-src"
              "rustc"
              "rustfmt"
            ])
            (rust.targets.aarch64-unknown-linux-gnu.fromManifestFile rust-manifest).rust-std
          ];

          mkShell =
            attrs:
            pkgs.mkShell (
              {
                BUNDLE_DIR = "build";
                MIRIFLAGS = "-Zmiri-deterministic-floats";
                LV2_BUNDLES = "${pkgs.callPackage ./nix/lv2.nix { inherit inputs; }}";
                SORD_VALIDATE_TOOL = "${pkgs.sord}/bin/sord_validate";
              }
              // attrs
              // extra
              // {
                packages = [
                  toolchain
                ]
                ++ lib.optional isLinux pkgs.gcc
                ++ attrs.packages or [ ]
                ++ extra.packages or [ ];
              }
            );
        in
        {
          default = mkShell (
            {
              packages = lib.optionals isLinux [
                pkgs.alsa-lib
                pkgs.libx11
                pkgs.libxcb
                pkgs.libxcursor
                pkgs.libxi
                pkgs.libxkbcommon
                pkgs.libxrandr
                pkgs.pkg-config
                pkgs.wayland
              ];

              HEADING_FONT = "${pkgs.barlow}/share/fonts/truetype/BarlowCondensed-SemiBold.ttf";
              MONO_FONT = "${pkgs.roboto-mono}/share/fonts/truetype/RobotoMono-Regular.ttf";
              MONO_MEDIUM_FONT = "${pkgs.roboto-mono}/share/fonts/truetype/RobotoMono-Medium.ttf";
            }
            // lib.optionalAttrs isLinux {
              ALSA_PLUGIN_DIR = "${pkgs.pipewire}/lib/alsa-lib";
            }
          );

          anagram = mkShell {
            packages = [
              pkgs.cargo-zigbuild
              pkgs.zig
            ];

            BUNDLE_DIR = "build-anagram";
            LV2LINT_TOOL = "${anagramPkgs.lv2lint}/bin/lv2lint";
            LV2_EMULATOR =
              if isLinux then
                anagramPkgs.stdenv.hostPlatform.emulator pkgs
              else
                lib.getExe (pkgs.callPackage ./nix/aarch64-linux-run { linuxPkgs = anagramPkgs; });

            ANAGRAM_TARGET = "aarch64-unknown-linux-gnu";
            DEVICE_GLIBC_VERSION = "2.34";
            NICKEL_LIB_DIR = "${nickel}/lib";
            LIBCLANG_PATH = "${lib.getLib pkgs.llvmPackages.libclang}/lib";
            NICKEL_INCLUDE_DIR = "${inputs.plugin-builder}/plugins-dep/package/libnickel";
            LV2_INCLUDE_DIR = "${pkgs.lv2.dev}/include";

            READELF_TOOL = "${pkgs.binutils-unwrapped}/bin/readelf";
            PATCHELF_TOOL = "${pkgs.patchelf}/bin/patchelf";
          };
        };

      devShells = extend: nixpkgs.lib.genAttrs systems (shells extend);
    in
    {
      lib = { inherit devShells; };
      devShells = devShells (_: { });
    };
}
