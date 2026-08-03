# SPDX-FileCopyrightText: 2026 Mohamed Hammad <Mohamed.Hammad@SpacecraftSoftware.org>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Development shell for `mcpctl`.
#
# Why this exists: the rustup toolchains under ~/.rustup on this machine are
# dynamically linked against /lib64/ld-linux-x86-64.so.2, which does not exist on
# NixOS, so `cargo` and `rustc` resolve on PATH but fail with "cannot execute:
# required file not found". The nixpkgs toolchain is patched correctly.
#
#   nix develop          # then: cargo build / cargo test / cargo clippy
{
  description = "mcpctl — MCP host config generator and deployer";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            clippy
            rustfmt
            rust-analyzer
            reuse # Standard §4.3 — `reuse lint` must stay clean
          ];
        };
      });

      packages = forAll (pkgs: {
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "mcpctl";
          version = "0.1.0";
          src = ./mcpctl;
          cargoLock.lockFile = ./mcpctl/Cargo.lock;
        };
      });
    };
}
