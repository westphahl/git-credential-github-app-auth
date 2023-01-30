# git-credential-github-app-auth

Git [credential
helper](https://git-scm.com/docs/gitcredentials#_custom_helpers) using GitHub
[App
authentication](https://docs.github.com/en/developers/apps/building-github-apps/authenticating-with-github-apps).


Configure the Git credential helper in `~/.gitconfig`:
```git
[credential "https://github.com"]
    helper = "github-app-auth"
    useHttpPath = true
```

Make sure the helper binary `git-credential-github-app-auth` is in your path
and the following environment variables are set:

```sh
export GITHUB_APP_ID=12345
export GITHUB_APP_PRIVATE_KEY=$(< /path/to/private/key.pem)
```
