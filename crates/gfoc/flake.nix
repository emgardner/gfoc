{
  description = "NexTORK Dev Shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix.url = "github:nix-community/fenix";
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
    ...
  }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];

    forAllSystems = f:
      nixpkgs.lib.genAttrs systems (
        system:
          f system
      );
  in {
    devShells = forAllSystems (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [fenix.overlays.default];
      };

      hostToolchain = pkgs.fenix.complete.withComponents [
        "cargo"
        "clippy"
        "rust-src"
        "rustc"
        "rustfmt"
        "llvm-tools-preview"
      ];

      rustToolchain = pkgs.fenix.combine [
        hostToolchain
        pkgs.fenix.targets.thumbv7em-none-eabi.latest.rust-std
      ];
    in {
      default = pkgs.mkShell {
        packages = with pkgs; [
          rustToolchain
          rust-analyzer-nightly
          cargo-binutils
          probe-rs-tools
          openocd
          pkg-config
        ];

        env = {
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
        };

        shellHook = ''
          echo "Embedded Rust + Embassy shell loaded"
          echo "rustc: $(rustc --version)"
          echo "cargo: $(cargo --version)"
        '';
      };
    });
  };
}
