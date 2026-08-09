# Remote BuildKit builder

Ignitify supports an optional remote BuildKit builder for application source builds. The control
plane keeps deployment and runtime on the single host; only the image build is sent to BuildKit.

## Provision the builder

Run a rootless or otherwise isolated `buildkitd` on the build host and expose it through a
TLS-protected TCP listener. Keep the CA, client certificate, and client key available to the
Ignitify administrator. Do not expose an unauthenticated BuildKit TCP port: BuildKit grants build
clients powerful access to the builder host.

The registry repository configured in the UI must be reachable by both BuildKit and the local
Docker runtime. Authenticate the Docker client on the Ignitify host with `docker login` when the
registry is private.

## Configure Ignitify

Open **Remote builders** from the admin navigation and enter:

- the `tcp://host:port` BuildKit endpoint;
- the registry repository, for example `registry.example.com/ignitify/builds`;
- the optional TLS server name;
- the CA certificate, client certificate, and client key PEM files.

Mark one builder as **Default**. New application builds then create a temporary Buildx remote
session, push an immutable digest-tagged image, and remove the session and temporary key material
after completion. Removing the default builder returns builds to the local executor.

The host **Concurrent builds** setting still limits the number of application builds in flight,
regardless of whether they run locally or on the remote BuildKit endpoint.
