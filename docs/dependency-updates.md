# Dependency Update Policy

Dependabot opens weekly version-update pull requests for the Rust workspace,
frontend, and GitHub Actions. GitHub Dependabot security updates remain enabled
separately through repository security settings and are not grouped by this
configuration.

Minor and patch updates for Rust and frontend dependencies are grouped to keep
review volume manageable. Major updates remain individual pull requests. GitHub
Actions updates are always individual because action revisions execute in the
release pipeline.

Every dependency pull request must pass the required CI checks. Do not enable
Dependabot auto-merge. Review the lockfile diff and the upstream changelog
before merging. For packages involved in cryptography, authentication, Docker,
Compose, SSH, Git, DNS, ingress, network transport, backup, or release
packaging, also confirm that the change preserves the relevant security
boundary and run focused regression coverage when the standard suite is not
sufficient.

For a security update, record the advisory, affected dependency path, chosen
version, verification result, and any accepted residual risk in the pull
request. Treat unavailable upstream fixes as tracked follow-up work rather
than locally patching transitive security-sensitive code.
