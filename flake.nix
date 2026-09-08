{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    devshell.url = "github:numtide/devshell";
    devshell.inputs.nixpkgs.follows = "nixpkgs";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, flake-utils, devshell, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ devshell.overlays.default rust-overlay.overlays.default ];
        };
        # Stable Rust with the wasm target and the tools CI runs (fmt, clippy)
        rust = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
          extensions = [ "rust-src" "rustfmt" "clippy" ];
        };
      in
      {
        devShell = pkgs.devshell.mkShell {
          packages = [
            rust
            pkgs.wasm-pack
            pkgs.wasm-bindgen-cli
            pkgs.binaryen
            pkgs.nodejs
            pkgs.github-cli
          ];
          commands = [
            { name = "build-wasm"; command = "wasm-pack build --release --target web"; help = "build pkg/ for the browser and Node"; }
            { name = "check"; command = "cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"; help = "what CI runs"; }
          ];
        };
      });
}
