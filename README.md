# mcp-servers

This repository contains MCP server configuration examples.

## Setup `mcp-nixos` with Nix flakes

Add `mcp-nixos` as a flake input and apply its overlay so `pkgs.mcp-nixos` is available:

```nix
# flake.nix
{
  inputs.mcp-nixos.url = "github:utensils/mcp-nixos";

  outputs = { nixpkgs, mcp-nixos, ... }: {
    nixpkgs.overlays = [ mcp-nixos.overlays.default ];
    # pkgs.mcp-nixos is now available everywhere
  };
}
```

Then configure your MCP client to run:

```json
{
  "command": "mcp-nixos"
}
```

For more information:

- https://mcp-nixos.io/
- https://mcp-nixos.io/usage