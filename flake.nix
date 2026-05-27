{
  description = "altair-rs - Rust utility crates with OpenTelemetry instrumentation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            cargo-llvm-cov
            cargo-deny
            cargo-nextest
            cargo-release
            release-plz
            go-task
            jq
            curl
            git
          ];

          shellHook = ''
            echo "altair-rs dev shell"
            echo "  rustc: $(rustc --version)"
            echo "  cargo: $(cargo --version)"
            echo "  task:  $(task --version 2>/dev/null || echo 'not found')"
          '';
        };
      });
}
