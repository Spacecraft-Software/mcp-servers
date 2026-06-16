# Contributing to mcp-servers

Thank you for your interest. Please read this document before opening an issue or pull
request — it sets honest expectations for both sides so no one's time is wasted.

## Project Stance

`mcp-servers` is a **personal hobby project** under the Spacecraft Software umbrella. It
is shaped around the maintainer's own use case — wiring a fixed set of MCP servers into
the coding agents and editors he uses — and developed at hobby pace. It is **not**
designed for general use, and it is not a community-driven project, but external input is
welcome and appreciated within the bounds set out below.

## What Is Welcome

- **Bug reports** — a config that doesn't load or a server that won't connect, with the
  host tool and version, the file, and the exact error.
- **New host support** — a config template for another MCP-capable CLI or editor, in that
  host's own dialect, following the layout in [`CLAUDE.md`](./CLAUDE.md).
- **Schema corrections** — a host changed its config format and a template is now stale.
- **Documentation fixes** — typos, inaccuracies, broken links, clarifications.

## What Is Not Guaranteed

- **PR acceptance.** Direction, scope, and which hosts are tracked are set by the
  maintainer alone. A submitted contribution is not a guaranteed merge, even if it is
  correct. If a PR is not accepted, that is a judgment of fit, not of the work.
- **Response time.** This is a hobby project. Expect responses on the order of days to
  weeks, not hours.
- **Roadmap influence.** Suggestions may inform direction but do not override the
  maintainer's plans.

## Before Opening a PR

1. **Open an issue first** for non-trivial changes (e.g., adding a new host). Discuss the
   approach before writing the config.
2. **Read [`CLAUDE.md`](./CLAUDE.md).** It documents the per-host schema table and the
   dialect traps (e.g., Qwen's `httpUrl`, Codex's `http_headers`, Copilot CLI's missing
   wrapper). New templates must match the host's real, current schema — verify with the
   host's own `mcp add` / `mcp list` where one exists.
3. **Keep the same server set across hosts.** A change to one server should be reflected
   in every host template in its respective dialect.
4. **Validate syntax** before submitting: well-formed JSON/JSONC/TOML/YAML for the files
   you touched.
5. **Never commit a real secret.** Use the `YOUR_CONTEXT7_API_KEY` placeholder (or the
   host's prompt/input mechanism). PRs containing live keys or tokens will be rejected.
6. **Keep REUSE coverage intact.** Every file must remain covered by `REUSE.toml`
   (or an inline SPDX header); `reuse lint` must pass.
7. **Sign your commits.** Cryptographic signing showing "Verified" is mandatory for this
   repo (Spacecraft Software Standard §6.3), and sign-off (`git commit -s`) under the
   [Developer Certificate of Origin](https://developercertificate.org/) is required.

## Commit Style

- Conventional Commits prefix (`feat:`, `fix:`, `docs:`, `chore:`).
- Subject ≤ 72 characters, imperative mood ("add" not "added").
- Body wrapped at 72 columns; explain *why*, not just *what*.
- Reference issues by number (`Closes #42`).
- New codenames (if any) follow the aerospace / sci-fi / AI naming convention
  (Spacecraft Software Standard §2). Host directories are named after the host tool and
  are functional identifiers, not codenames.

## Forking

If your needs diverge from the maintainer's, **fork it**. That is exactly what
GPL-3.0-or-later is for. The only constraints are those imposed by the license itself:
keep the source open and under a compatible license, preserve copyright notices, and pass
the same freedoms downstream.

## Reporting Security Issues

For security-sensitive issues (e.g., a template that leaks credentials), do **not** open a
public issue. Email &lt;Mohamed.Hammad@SpacecraftSoftware.org&gt; with details. PGP key
available on request. A coordinated-disclosure window of 90 days from acknowledgment is
the default; this can be shortened or lengthened by mutual agreement.

## License of Contributions

By submitting a contribution, you agree that it will be licensed under
**GPL-3.0-or-later**, the same terms as the project. Contributions that cannot be licensed
under GPL-3.0-or-later cannot be accepted. You retain copyright in your contributions; no
CLA is required.

---

**Maintainer:** Mohamed Hammad &lt;Mohamed.Hammad@SpacecraftSoftware.org&gt;
**License:** GPL-3.0-or-later
**Website:** <https://SpacecraftSoftware.org/>

*--- Forged in Spacecraft Software ---*
