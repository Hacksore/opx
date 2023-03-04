# opx

Sourcing all `.env` files from your working directory.

```
# start your app with secrets injected
opx
```
The command above would run this in the background:
```
op run <expandedEnvFiles> -- <pkgManager> <script || "start">
```

### Demo
Working example of it doing the correct thing in a demo repo:
```
master ✗ $ opx
Running yarn start with secrets expanded from .env files
yarn run v1.22.19
$ turbo run start
• Packages in scope: demo, eslint-config-custom, other-app, tsconfig
• Running start in 4 packages
• Remote caching disabled
demo:start: cache bypass, force executing 545833253ebd38cc
other-app:start: cache bypass, force executing 2ed51133d14970ce
demo:start:
other-app:start:
other-app:start: > other-app@1.0.0 start
other-app:start: > node main.js
other-app:start:
demo:start: > demo@1.0.0 start
demo:start: > node main.js
demo:start:
other-app:start: Hello this is a sample app that uses a secret from 1password cli
other-app:start: Secret is: <concealed by 1Password>
demo:start: Hello this is a sample app that uses a secret from 1password cli
demo:start: Secret is: <concealed by 1Password>

 Tasks:    2 successful, 2 total
Cached:    0 cached, 2 total
  Time:    315ms

Done in 0.56s.
```

### Considerations
- How do you handle duplicate env vars?
- How do you handle different environment dimensions (.env.local vs .env.production, etc)

### TODO
- [ ] Allow a user defined node package manager 
- [ ] Support for more than just the node ecosystem
- [ ] Allow a custom script to be passed in