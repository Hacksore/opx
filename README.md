# onepassword-secret-util

This tool allows you to use the `opx` binary to start an application with all `.env` files passed to `op run ...`. 

### Install
`cargo install onepassword-secret-util`

```
# start your app with secrets injected
opx
```
The command above would run this in the background:
```
op run --env-file=.env --env-file=apps/web/.env -- npm start
```

### Demo
Working example of it doing the correct thing in a demo repo:
```
opx ✔ $ opx
[OPX] Can't find config file "opx.json", using default values
[OPX] Forcing terminal colors with FORCE_COLOR=1
[ENV] .env
[ENV] apps/demo/.env
[ENV] apps/other-app/.env
[OPX] op run --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/.env --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/apps/demo/.env --env-file=/Users/hacksore/Code/opensource/demo-1pass-secrets/apps/other-app/.env -- npm start

> demo-1pass-secrets@0.0.0 start
> turbo run start

• Packages in scope: demo, eslint-config-custom, other-app, tsconfig
• Running start in 4 packages
• Remote caching disabled
demo:start: cache bypass, force executing 545833253ebd38cc
other-app:start: cache bypass, force executing 2ed51133d14970ce
other-app:start:
other-app:start: > other-app@1.0.0 start
other-app:start: > node main.js
other-app:start:
demo:start:
demo:start: > demo@1.0.0 start
demo:start: > node main.js
demo:start:
demo:start: Hello this is a sample app that uses a secret from 1password cli
other-app:start: Hello this is a sample app that uses a secret from 1password cli
demo:start: Secret is: <concealed by 1Password>
other-app:start: Secret is: <concealed by 1Password>

 Tasks:    2 successful, 2 total
Cached:    0 cached, 2 total
  Time:    216ms
```

### Considerations
- How do you handle duplicate env vars?
- How do you handle different environment dimensions (.env.local vs .env.production, etc)

### TODO
- [ ] Allow a user defined node package manager 
- [ ] Support for more than just the node ecosystem
- [x] Allow a custom script to be passed in