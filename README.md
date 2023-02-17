# git-credential-github-app-auth

Git [credential
helper](https://git-scm.com/docs/gitcredentials#_custom_helpers) using GitHub
[App
authentication](https://docs.github.com/en/developers/apps/building-github-apps/authenticating-with-github-apps)
to provide Github tokens as credentials to Git.

The helper will cache the credentials and only request a new token when the
previous one expired.

## Setup

Create a [Github
App](https://docs.github.com/en/apps/creating-github-apps/creating-github-apps/creating-a-github-app)
and install it on the repositor or account/organization.

The app needs to have at least read-only
[permission](https://docs.github.com/en/apps/maintaining-github-apps/editing-a-github-apps-permissions)
for the "Contents" of the repository.

You must also [generate a private
key](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-with-github-apps#generating-a-private-key)
for the app in order to make API requests.

## Usage

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

To test that the authentication helper work, you can either clone a repo that
has the configured Github app installed or run the client from the command line
and providing the input via stdin:

```sh
echo "protocol=https\nhost=github.com\npath=westphahl/git-credential-github-app-auth.git\n\n" \
    | git-credential-github-app-auth \
        /run/user/1000/github-app-auth \
        client get
```
