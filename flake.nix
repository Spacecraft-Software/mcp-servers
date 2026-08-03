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

      packages = forAll (pkgs:
        let
          mcpctl = pkgs.rustPlatform.buildRustPackage {
            pname = "mcpctl";
            version = "0.1.0";

            # The whole repository, not just ./mcpctl: the round-trip tests in
            # mcpctl/tests/render.rs resolve CARGO_MANIFEST_DIR/.. and load the real
            # `mcp.toml` and the real templates. Vendoring only the crate makes
            # `cargo test` fail in the check phase with "cannot read /build/mcp.toml".
            src = ./.;
            cargoRoot = "mcpctl"; # where Cargo.lock lives
            buildAndTestSubdir = "mcpctl"; # where cargo build/test run

            cargoLock.lockFile = ./mcpctl/Cargo.lock;
          };
        in
        {
          inherit mcpctl;
          # `nix profile add .#mcpctl` should work as well as `.#default`.
          default = mcpctl;
        });
    };
}
