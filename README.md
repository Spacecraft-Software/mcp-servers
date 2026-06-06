# mcp-servers

This repository contains MCP server configuration examples.

## Setup `mcp-nixos` with Nix flakes

Add `mcp-nixos` as a flake input and apply its overlay when importing `nixpkgs`:

```nix
# flake.nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.mcp-nixos.url = "github:utensils/mcp-nixos";

  outputs = { nixpkgs, mcp-nixos, ... }:
  let
    system = "x86_64-linux";
    pkgs = import nixpkgs {
      inherit system;
      overlays = [ mcp-nixos.overlays.default ];
    };
  in {
    packages.${system}.mcp-nixos = pkgs.mcp-nixos;
  };
}
```

`mcp-nixos` will be available in the contexts that use this `pkgs` set.

Then configure your MCP client to run `mcp-nixos` (with that package on `PATH`):

```json
{
  "command": "mcp-nixos"
}
```

For more information:

- https://mcp-nixos.io/
- https://mcp-nixos.io/usage