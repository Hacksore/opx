# PRD: Environment Selection for opx

## Summary

`opx` should support selecting an application environment, such as `prod`, `staging`, or `dev`, and automatically load the matching 1Password-backed env files before running the requested package manager command.

Target example:

```sh
opx db:push --prod
```

The command should resolve `--prod` to the production environment, load the correct env files, and run the package manager command through `op run` without requiring users to hard-code or manually swap env files.

## Problem

Today, `opx` recursively finds files named exactly `.env` and passes them to `op run`:

```sh
op run --env-file=.env --env-file=apps/web/.env -- pnpm start
```

This works for a single environment, but it does not handle common cases where the same app needs different credentials for local, preview, staging, and production.

Current workaround:

```sh
export UPLOADTHING_APP_ID="op://typehero/uploadthing-app-id/credential"
export UPLOADTHING_SECRET="op://typehero/uploadthing-secret/credential"
```

Users can manually maintain different files, but `opx` has no environment model and no way to know which files should be used for a command like `db:push`.

## Goals

- Let users select an environment from the CLI.
- Keep the default zero-config behavior: `opx` should continue loading existing `.env` files.
- Support ergonomic aliases like `--prod`.
- Support explicit environment selection like `--env prod`.
- Make loaded files visible in command output.
- Avoid leaking resolved secret values.
- Keep the model compatible with native 1Password secret references in env files.
- Define clear precedence when multiple files define the same variable.

## Non-Goals

- Building a full secrets manager.
- Resolving 1Password references directly inside `opx`.
- Replacing `op run`.
- Automatically creating vaults or secrets in 1Password.
- Supporting every dotenv convention from every framework on day one.

## Proposed UX

### Default

```sh
opx
opx dev
opx db:push
```

Without an environment flag, `opx` keeps current behavior and loads files named `.env`.

### Explicit Environment

```sh
opx --env prod db:push
opx db:push --env prod
```

`opx` consumes `--env prod`, selects environment-specific files, and does not forward `--env prod` to the package manager command.

### Alias Flags

```sh
opx db:push --prod
opx db:push --staging
opx db:push --dev
```

Aliases are shorthand for `--env <name>`.

Recommended built-in aliases:

| Flag | Environment |
| --- | --- |
| `--dev` | `dev` |
| `--local` | `local` |
| `--preview` | `preview` |
| `--staging` | `staging` |
| `--prod` | `prod` |
| `--production` | `production` |

### Escaping opx Flags

If a downstream script needs to receive a flag that looks like an `opx` environment alias, users can place command arguments after `--`:

```sh
opx --env prod -- db:push --prod
```

In this case, only `--env prod` is consumed by `opx`; everything after `--` is forwarded to the package manager.

## Recommended File Convention

Support recursive discovery of these files:

```txt
.env
.env.local
.env.dev
.env.staging
.env.prod
.env.production
apps/web/.env
apps/web/.env.prod
packages/api/.env
packages/api/.env.prod
```

For `opx db:push --prod`, load:

```txt
.env
.env.prod
apps/web/.env
apps/web/.env.prod
packages/api/.env
packages/api/.env.prod
```

This makes `.env` the shared baseline and `.env.<environment>` the selected overlay.

## File Precedence

Recommended precedence, from lowest to highest:

1. Root `.env`
2. Nested `.env`
3. Root `.env.<environment>`
4. Nested `.env.<environment>`

If duplicate variables appear, later files win according to the order passed to `op run`.

`opx` should print duplicate variable names when detected, without printing values:

```txt
[OPX] Duplicate env vars detected; later files win:
[OPX] DATABASE_URL: .env, apps/web/.env.prod
```

## 1Password Secret Reference Patterns

### Option A: Separate Secret Names

```sh
DATABASE_URL="op://typehero/database-url-prod/credential"
```

Pros:

- Works with 1Password today.
- Easy to reason about from a single env file.
- No special `opx` interpolation.

Cons:

- Repeats the same variable names across files.
- Secret naming conventions become important.
- Renaming environments may require many file edits.

### Option B: Separate Vaults

```sh
DATABASE_URL="op://typehero-prod/database-url/credential"
```

Pros:

- Strong separation by environment.
- Easier to grant production access separately from development.
- Secret names can stay stable across environments.

Cons:

- Requires more vault setup.
- Moving secrets between vaults is operationally heavier.
- Users need to understand vault-level access controls.

### Option C: One File with opx Interpolation

```sh
DATABASE_URL="op://typehero-${OPX_ENV}/database-url/credential"
```

Pros:

- Reduces duplicate env files.
- Environment mapping is compact.
- Secret names can stay stable.

Cons:

- Adds custom behavior before `op run`.
- Harder to inspect exactly what `op` receives.
- Creates escaping and validation edge cases.
- More likely to surprise users familiar with native `op run`.

Recommendation: avoid interpolation for the first implementation. Prefer native `op://` references inside `.env.<environment>` files.

## Configuration Options

### Option 1: Convention Only

Use file names only:

```txt
.env
.env.prod
apps/web/.env
apps/web/.env.prod
```

Pros:

- Smallest implementation.
- No extra config file.
- Matches existing `opx` behavior.
- Easy to document.

Cons:

- Harder to customize aliases.
- Ambiguous when users prefer `.env.production` over `.env.prod`.
- No per-environment include/exclude control.

### Option 2: Config File

Add an `opx.config.json` or `opx.config.toml`:

```json
{
  "environments": {
    "prod": {
      "aliases": ["production"],
      "files": [".env", ".env.prod", "apps/*/.env", "apps/*/.env.prod"]
    },
    "staging": {
      "files": [".env", ".env.staging", "apps/*/.env", "apps/*/.env.staging"]
    }
  }
}
```

Pros:

- Explicit.
- Supports aliases cleanly.
- Can model monorepos with unusual layouts.
- Can grow later to include duplicate handling, package-manager defaults, or skip dirs.

Cons:

- More setup.
- More schema and validation work.
- Another file for users to maintain.

### Option 3: Hybrid

Use conventions by default and optionally allow config overrides.

Pros:

- Preserves zero-config behavior.
- Gives larger repos an escape hatch.
- Allows implementation to start simple and grow later.

Cons:

- Requires documenting both convention and config.
- Needs clear precedence between convention and config.

Recommendation: use the hybrid approach. Ship convention support first, then add config only when a repo needs to override default behavior.

## Recommended MVP

Implement convention-based environment selection:

- Parse `--env <name>` and `--env=<name>`.
- Parse built-in aliases like `--prod`.
- Strip consumed `opx` flags before forwarding command args.
- Preserve everything after `--` as package manager args.
- Discover `.env` files plus `.env.<selected-env>` files.
- For `prod`, also consider `.env.production`.
- Print selected environment:

```txt
[OPX] Environment: prod
```

- Print loaded files in final precedence order.
- Warn on duplicate variable names.
- Keep current behavior when no environment is selected.

## Candidate Selection Algorithm

Given:

```sh
opx db:push --prod
```

1. Parse CLI args.
2. Resolve environment:
   - `--prod` maps to `prod`.
   - Remove `--prod` from forwarded args.
3. Discover candidate files recursively, skipping `.git` and `node_modules`.
4. Include files whose names are:
   - `.env`
   - `.env.prod`
   - `.env.production`
5. Sort files by precedence:
   - baseline `.env` before environment files
   - shallower paths before deeper paths
   - stable lexical ordering within each group
6. Run:

```sh
op run --env-file=.env --env-file=.env.prod -- pnpm db:push
```

## Open Questions

- Should `--prod` map to `prod`, `production`, or both?
- Should `.env.local` always load for no selected environment, or only for `--local`?
- Should duplicate variables be warnings, hard errors, or configurable?
- Should `opx` support `OPX_ENV=prod opx db:push` as a non-flag selector?
- Should package-specific commands only load env files for the relevant workspace package?
- Should selected environment aliases be user-configurable in the first release?

## Risks

- `--prod` may be intended for the underlying script, not for `opx`.
- Loading both `.env.prod` and `.env.production` can create confusing duplicates.
- Monorepos may accidentally load unrelated app secrets.
- Recursive env discovery can become expensive in large repos.
- A bad precedence model can make production commands silently use development credentials.

## Mitigations

- Treat `--env <name>` as the canonical form and document aliases as convenience.
- Support `--` to force downstream argument forwarding.
- Print the selected environment and env file list every time.
- Add duplicate variable warnings before running the command.
- Keep aliases small and predictable.
- Consider a future `opx.config.*` for repos that need stricter control.

## Success Criteria

- A user can run `opx db:push --prod` and see production env files loaded.
- Existing `opx`, `opx dev`, and `opx db:push` behavior remains unchanged.
- The final printed `op run` command clearly shows which env files were passed.
- Duplicate variable warnings do not reveal secret values.
- Documentation explains the tradeoffs between separate secret names, separate vaults, and config-driven mapping.

