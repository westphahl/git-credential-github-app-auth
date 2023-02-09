# git-credential-github-app-auth

Git [credential
helper](https://git-scm.com/docs/gitcredentials#_custom_helpers) using GitHub
[App
authentication](https://docs.github.com/en/developers/apps/building-github-apps/authenticating-with-github-apps).

Make sure the helper binary `git-credential-github-app-auth` is in your path.
The authentication agent listens on a local Unix socket and can be started with
the following command:

```sh
git-credential-github-app-auth \
    /run/user/1000/github-app-auth \
    agent \
    --app-id 1234 \
    --key-path /path/to/app/private-key.pem
```

Configure the Git credential helper in `~/.gitconfig`:

```git
[credential "https://github.com"]
    helper = "github-app-auth /run/user/1000/github-app-auth client"
    useHttpPath = true
```

To test the authentication helper you can either clone a repo that has the
configured Github app installed or run the client from the command line and
providing the input via stdin:

```sh
echo "protocol=https\nhost=github.com\npath=westphahl/git-credential-github-app-auth.git\n\n" \
    | git-credential-github-app-auth \
        /run/user/1000/github-app-auth \
        client get
```
