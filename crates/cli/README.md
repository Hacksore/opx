# opx

This tool allows you to use the `opx` binary to start an application with project `.env` files passed to `op run ...`.

### Install
`cargo install opx`

```
# start your app with secrets injected
opx
```
The command above would run this in the background:
```
op run --env-file=.env --env-file=apps/web/.env -- npm run dev
```

By default, `opx` runs the `dev` package script. You can override this in `package.json`:

```json
{
  "opx": {
    "defaultScript": "start"
  }
}
```

Explicit command args still take precedence:

```sh
opx start
opx run server
```

### Environments

By default, `opx` only loads files named `.env`.

```sh
opx db:push
```

To load a stage-specific env file, pass `--env <stage>`:

```sh
opx --env prod db:push
```

The command above loads `.env.prod` files instead of `.env` files. The shorthand flags `--prod`, `--dev`, and `--staging` are also supported:

```sh
opx --prod db:push
opx --dev dev
opx --staging start
```

If the command you are running needs its own `--prod` flag, put command args after `--`:

```sh
opx --env prod -- db:push --prod
```

### Demo
Working example of it doing the correct thing in a demo repo:
```
opx ✔ $ opx
[OPX] Forcing terminal colors with FORCE_COLOR=1
[ENV] .env
[ENV] apps/demo/.env
[ENV] apps/other-app/.env
[OPX] op run --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/.env --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/apps/demo/.env --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/apps/other-app/.env -- npm run dev

> demo-1pass-secrets@0.0.0 dev
> turbo run dev

• Packages in scope: demo, eslint-config-custom, other-app, tsconfig
• Running dev in 4 packages
• Remote caching disabled
demo:dev: cache bypass, force executing 545833253ebd38cc
other-app:dev: cache bypass, force executing 2ed51133d14970ce
other-app:dev:
other-app:dev: > other-app@1.0.0 dev
other-app:dev: > node main.js
other-app:dev:
demo:dev:
demo:dev: > demo@1.0.0 dev
demo:dev: > node main.js
demo:dev:
demo:dev: Hello this is a sample app that uses a secret from 1password cli
other-app:dev: Hello this is a sample app that uses a secret from 1password cli
demo:dev: Secret is: <concealed by 1Password>
other-app:dev: Secret is: <concealed by 1Password>

 Tasks:    2 successful, 2 total
Cached:    0 cached, 2 total
  Time:    216ms
```

# Debug
How i link it

```
# dev
cargo watch -x "build --release"

# link it
export PATH="$HOME/code/opx/crates/cli/target/release:$PATH"
```

### Considerations
- How do you handle duplicate env vars?
- Should `.env.production` alias `.env.prod`?
