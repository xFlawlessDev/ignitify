#!/usr/bin/env bash
# Installed inside each release bundle by scripts/package-release.sh.
set -Eeuo pipefail
IFS=$'\n\t'

readonly APP_NAME="ignitify"
readonly SERVICE_NAME="ignitify.service"
readonly SERVICE_USER="ignitify"
readonly SERVICE_GROUP="ignitify"

PREFIX="/opt/ignitify"
DATA_DIR="/var/lib/ignitify"
CONFIG_DIR="/etc/ignitify"
INSTALL_PREREQUISITES=1
INSTALL_INGRESS_ASSETS=1
START_SERVICE=1
ENABLE_LOCAL_BUILDS=1

usage() {
  cat <<'EOF'
Usage: install [options]

Installs an extracted Ignitify release bundle on a systemd Linux host. By
default, it installs all required runtime dependencies, grants the dedicated
service account Docker socket access, enables local Git source builds and
Traefik ingress, then enables and starts Ignitify.

Options:
  --prefix PATH             Application directory (default: /opt/ignitify)
  --data-dir PATH           Persistent data directory (default: /var/lib/ignitify)
  --config-dir PATH         Environment-file directory (default: /etc/ignitify)
  --no-prerequisites        Do not install or configure Docker, Git, or OpenSSH.
  --no-ingress-assets       Do not install the bundled Traefik operator assets.
  --no-local-builds         Keep local Docker-host source builds disabled.
  --no-start                Install but do not enable or start Ignitify.
  -h, --help                Show this help text.

Automatic prerequisite installation is supported on Ubuntu, Debian, and Fedora.
It uses Docker's official package repository and refuses to remove an existing,
conflicting Docker installation or its workloads.
EOF
}

info() {
  printf '%s\n' "[ignitify] $*"
}

warn() {
  printf '%s\n' "[ignitify] warning: $*" >&2
}

die() {
  printf '%s\n' "[ignitify] error: $*" >&2
  exit 1
}

need_value() {
  local option="$1"
  local value="${2:-}"
  [[ -n "$value" ]] || die "$option requires a value"
  printf '%s' "$value"
}

validate_absolute_path() {
  local name="$1"
  local value="$2"
  [[ "$value" == /* && "$value" != / ]] || die "$name must be an absolute path other than /"
  [[ "$value" != *$'\n'* && "$value" != *' '* ]] || die "$name cannot contain whitespace"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command is unavailable: $1"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --prefix)
      PREFIX="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --data-dir)
      DATA_DIR="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --config-dir)
      CONFIG_DIR="$(need_value "$1" "${2:-}")"
      shift 2
      ;;
    --no-prerequisites)
      INSTALL_PREREQUISITES=0
      shift
      ;;
    --no-ingress-assets)
      INSTALL_INGRESS_ASSETS=0
      shift
      ;;
    --no-local-builds)
      ENABLE_LOCAL_BUILDS=0
      shift
      ;;
    --no-start)
      START_SERVICE=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

[[ "$(uname -s)" == "Linux" ]] || die "this installer supports Linux release bundles only"
[[ "${EUID}" -eq 0 ]] || die "run the release installer as root"
validate_absolute_path "--prefix" "$PREFIX"
validate_absolute_path "--data-dir" "$DATA_DIR"
validate_absolute_path "--config-dir" "$CONFIG_DIR"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
CONFIG_FILE="$CONFIG_DIR/ignitify.env"
BIN_DIR="$PREFIX/bin"
CORE_BINARY="$BIN_DIR/ignitify-core"
LAUNCHER="$BIN_DIR/ignitify"
SYSTEMD_UNIT="/etc/systemd/system/$SERVICE_NAME"
INGRESS_SOURCE="$SCRIPT_DIR/infra/traefik"

[[ -f "$SCRIPT_DIR/ignitify-core" ]] || die "release bundle is missing ignitify-core"
[[ -f "$SCRIPT_DIR/railpack" ]] || die "release bundle is missing railpack; publish a complete bundle to enable all source build modes"
if [[ "$INSTALL_INGRESS_ASSETS" -eq 1 && ! -f "$INGRESS_SOURCE/compose.yaml" ]]; then
  die "release bundle is missing infra/traefik/compose.yaml"
fi

installed_debian_conflicts() {
  local package
  local conflicts=()
  for package in docker.io docker-compose docker-compose-v2 docker-doc docker-buildx podman-docker containerd runc; do
    if dpkg-query -W -f='${db:Status-Status}' "$package" 2>/dev/null | grep -qx installed; then
      conflicts+=("$package")
    fi
  done
  ((${#conflicts[@]} == 0)) || die "existing Docker packages conflict with Docker Engine: ${conflicts[*]}. Remove or migrate them manually; this installer will not alter existing Docker workloads."
}

installed_fedora_conflicts() {
  local package
  local conflicts=()
  for package in docker docker-client docker-client-latest docker-common docker-latest docker-latest-logrotate docker-logrotate podman-docker runc; do
    if rpm -q "$package" >/dev/null 2>&1; then
      conflicts+=("$package")
    fi
  done
  ((${#conflicts[@]} == 0)) || die "existing Docker packages conflict with Docker Engine: ${conflicts[*]}. Remove or migrate them manually; this installer will not alter existing Docker workloads."
}

install_apt_prerequisites() {
  local distribution="$1"
  local codename
  local legacy_source="/etc/apt/sources.list.d/docker.list"
  local legacy_source_pattern
  local legacy_source_allowed_pattern

  require_command apt-get
  require_command dpkg
  installed_debian_conflicts

  # Older Docker setup instructions use docker.list. Remove only that known
  # official Docker source so it does not duplicate the canonical deb822 file.
  legacy_source_pattern="^[[:space:]]*deb([[:space:]]|\\[)[^#]*download[.]docker[.]com/linux/${distribution}([[:space:]/]|$)"
  legacy_source_allowed_pattern="^[[:space:]]*(#|$|deb([[:space:]]|\\[)[^#]*download[.]docker[.]com/linux/${distribution}([[:space:]/]|$))"
  if [[ -f "$legacy_source" ]] \
    && grep -Eqs "$legacy_source_pattern" "$legacy_source" \
    && ! grep -E -v -q "$legacy_source_allowed_pattern" "$legacy_source"; then
    rm -f -- "$legacy_source"
  fi

  apt-get update
  apt-get install -y ca-certificates curl git gnupg openssh-client
  install -m 0755 -d /etc/apt/keyrings
  curl -fsSL "https://download.docker.com/linux/$distribution/gpg" -o /etc/apt/keyrings/docker.asc
  chmod a+r /etc/apt/keyrings/docker.asc
  codename="${UBUNTU_CODENAME:-${VERSION_CODENAME:-}}"
  [[ -n "$codename" ]] || die "could not determine the $distribution release codename"

  cat > /etc/apt/sources.list.d/docker.sources <<EOF
Types: deb
URIs: https://download.docker.com/linux/$distribution
Suites: $codename
Components: stable
Architectures: $(dpkg --print-architecture)
Signed-By: /etc/apt/keyrings/docker.asc
EOF
  apt-get update
  # A host may already have official Docker packages held with apt-mark hold.
  # These are the exact packages this installer manages, so allow their update.
  apt-get install -y --allow-change-held-packages docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
}

install_fedora_prerequisites() {
  require_command dnf
  installed_fedora_conflicts
  dnf install -y ca-certificates curl git openssh-clients
  curl -fsSL https://download.docker.com/linux/fedora/docker-ce.repo -o /etc/yum.repos.d/docker-ce.repo
  dnf install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
}

install_prerequisites() {
  [[ "$INSTALL_PREREQUISITES" -eq 1 ]] || return 0
  [[ -r /etc/os-release ]] || die "could not identify the Linux distribution"

  # shellcheck disable=SC1091
  source /etc/os-release
  case "$ID" in
    ubuntu|debian) install_apt_prerequisites "$ID" ;;
    fedora) install_fedora_prerequisites ;;
    *) die "automatic prerequisite installation supports Ubuntu, Debian, and Fedora only; install Docker Engine with Compose and Buildx, Git, OpenSSH client, and systemd first, then re-run with --no-prerequisites" ;;
  esac
}

validate_prerequisites() {
  local command
  for command in awk docker git ssh ssh-keygen systemctl; do
    require_command "$command"
  done
  docker compose version >/dev/null
  docker buildx version >/dev/null
  systemctl enable --now docker
  docker info --format '{{.ServerVersion}}' >/dev/null
}

ensure_service_account() {
  if ! getent group "$SERVICE_GROUP" >/dev/null; then
    groupadd --system "$SERVICE_GROUP"
  fi
  if ! id -u "$SERVICE_USER" >/dev/null 2>&1; then
    useradd \
      --system \
      --gid "$SERVICE_GROUP" \
      --home-dir "$DATA_DIR" \
      --no-create-home \
      --shell /usr/sbin/nologin \
      "$SERVICE_USER"
  fi
  getent group docker >/dev/null || die "Docker did not create the docker group"
  usermod --append --groups docker "$SERVICE_USER"
}

install_ingress_assets() {
  local target="$PREFIX/infra/traefik"
  local traefik_data_dir="$DATA_DIR/traefik"
  local dynamic_dir="$DATA_DIR/traefik/dynamic"
  local fallback_dir="$DATA_DIR/traefik/fallback"
  local fallback_page="$fallback_dir/404.html"
  local required

  for required in \
    compose.yaml \
    entrypoint.sh \
    traefik.yaml \
    fallback/Caddyfile \
    fallback/404.html \
    fallback/ignitify-mark.svg \
    socket-proxy/Dockerfile \
    socket-proxy/entrypoint.sh \
    dynamic/fallback.yml \
    dynamic/middlewares.yml; do
    [[ -f "$INGRESS_SOURCE/$required" ]] || die "release ingress asset is missing: infra/traefik/$required"
  done

  install -d -o root -g root -m 0755 "$target/fallback" "$target/socket-proxy"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/compose.yaml" "$target/compose.yaml"
  install -o root -g root -m 0755 "$INGRESS_SOURCE/entrypoint.sh" "$target/entrypoint.sh"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/traefik.yaml" "$target/traefik.yaml"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/fallback/Caddyfile" "$target/fallback/Caddyfile"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/fallback/404.html" "$target/fallback/404.html"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/fallback/ignitify-mark.svg" "$target/fallback/ignitify-mark.svg"
  install -o root -g root -m 0644 "$INGRESS_SOURCE/socket-proxy/Dockerfile" "$target/socket-proxy/Dockerfile"
  install -o root -g root -m 0755 "$INGRESS_SOURCE/socket-proxy/entrypoint.sh" "$target/socket-proxy/entrypoint.sh"

  # Reapply ownership and modes on every install. `install -d` only applies
  # attributes while creating a directory, so an upgrade must repair paths
  # left behind by an older or manual installation.
  install -d "$traefik_data_dir" "$dynamic_dir" "$dynamic_dir/certs" "$fallback_dir"
  chown "$SERVICE_USER:$SERVICE_GROUP" "$traefik_data_dir" "$dynamic_dir" "$dynamic_dir/certs" "$fallback_dir"
  chmod 0700 "$traefik_data_dir" "$dynamic_dir" "$dynamic_dir/certs"
  chmod 0755 "$fallback_dir"
  install -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0644 "$INGRESS_SOURCE/dynamic/fallback.yml" "$dynamic_dir/fallback.yml"
  install -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0644 "$INGRESS_SOURCE/dynamic/middlewares.yml" "$dynamic_dir/middlewares.yml"
  if [[ -e "$fallback_page" && ! -f "$fallback_page" ]]; then
    die "runtime fallback page is not a regular file: $fallback_page"
  fi
  if [[ ! -e "$fallback_page" ]]; then
    install -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0644 "$INGRESS_SOURCE/fallback/404.html" "$fallback_page"
  else
    chown "$SERVICE_USER:$SERVICE_GROUP" "$fallback_page"
    chmod 0644 "$fallback_page"
  fi
}

generate_secret() {
  od -An -N32 -tx1 /dev/urandom | tr -d ' \n'
}

write_default_config() {
  local bootstrap_secret
  local ingress_auto_start=false
  local local_builds=false
  bootstrap_secret="$(generate_secret)"
  [[ "${#bootstrap_secret}" -eq 64 ]] || die "could not generate a bootstrap secret"
  if [[ "$INSTALL_INGRESS_ASSETS" -eq 1 ]]; then
    ingress_auto_start=true
  fi
  if [[ "$ENABLE_LOCAL_BUILDS" -eq 1 ]]; then
    local_builds=true
  fi

  umask 077
  cat > "$CONFIG_FILE" <<EOF
# Generated by the Ignitify release installer. Keep this file readable only by root.
# The bootstrap secret is required only to create the first platform operator.
IGNITIFY_LISTEN_ADDR=127.0.0.1:5656
IGNITIFY_SECURE_COOKIES=true
IGNITIFY_BOOTSTRAP_SECRET=$bootstrap_secret
IGNITIFY_DATA_DIR=$DATA_DIR
IGNITIFY_DATABASE_URL=sqlite:$DATA_DIR/ignitify.db
IGNITIFY_COMPOSE_ROOT=$DATA_DIR/compose
IGNITIFY_SOURCE_BUILD_ROOT=$DATA_DIR/builds
IGNITIFY_TRAEFIK_DYNAMIC_DIR=$DATA_DIR/traefik/dynamic
IGNITIFY_AUTO_START_INGRESS=$ingress_auto_start
IGNITIFY_ALLOW_LOCAL_BUILDS=$local_builds
# Configure a TLS reverse proxy, trusted HTTPS origins, ACME email, and domains before remote access.
EOF
  if [[ "$INSTALL_INGRESS_ASSETS" -eq 1 ]]; then
    printf 'IGNITIFY_TRAEFIK_COMPOSE_FILE=%s\n' "$PREFIX/infra/traefik/compose.yaml" >> "$CONFIG_FILE"
    printf 'IGNITIFY_TRAEFIK_FALLBACK_PAGE_FILE=%s\n' "$DATA_DIR/traefik/fallback/404.html" >> "$CONFIG_FILE"
  fi
  chown root:root "$CONFIG_FILE"
  chmod 0600 "$CONFIG_FILE"
}

ensure_ingress_runtime_config() {
  [[ "$INSTALL_INGRESS_ASSETS" -eq 1 ]] || return 0

  if awk '/^[[:space:]]*IGNITIFY_TRAEFIK_FALLBACK_PAGE_FILE=/{found=1} END {exit !found}' "$CONFIG_FILE"; then
    return 0
  fi

  printf '\nIGNITIFY_TRAEFIK_FALLBACK_PAGE_FILE=%s\n' "$DATA_DIR/traefik/fallback/404.html" >> "$CONFIG_FILE"
  chown root:root "$CONFIG_FILE"
  chmod 0600 "$CONFIG_FILE"
  info "configured runtime Traefik fallback page"
}

write_launcher() {
  cat > "$LAUNCHER" <<EOF
#!/usr/bin/env bash
set -Eeuo pipefail
set -a
source "$CONFIG_FILE"
set +a
exec "$CORE_BINARY" "\$@"
EOF
  chown root:root "$LAUNCHER"
  chmod 0750 "$LAUNCHER"
}

write_systemd_unit() {
  local temporary_unit
  temporary_unit="$(mktemp)"
  trap 'rm -f "$temporary_unit"' RETURN
  cat > "$temporary_unit" <<EOF
[Unit]
Description=Ignitify deployment control plane
Wants=network-online.target
After=network-online.target docker.service
Requires=docker.service

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_GROUP
WorkingDirectory=$PREFIX
EnvironmentFile=$CONFIG_FILE
Environment=PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
ExecStart=$CORE_BINARY
Restart=on-failure
RestartSec=5s
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=full
ReadWritePaths=$DATA_DIR

[Install]
WantedBy=multi-user.target
EOF
  install -o root -g root -m 0644 "$temporary_unit" "$SYSTEMD_UNIT"
  rm -f "$temporary_unit"
  trap - RETURN
}

link_launcher_if_available() {
  local global_launcher="/usr/local/bin/$APP_NAME"
  if [[ -L "$global_launcher" ]]; then
    ln -sfn "$LAUNCHER" "$global_launcher"
  elif [[ ! -e "$global_launcher" ]]; then
    ln -s "$LAUNCHER" "$global_launcher"
  else
    warn "not creating $global_launcher because it already exists and is not a symlink"
  fi
}

install_prerequisites
for command in chmod chown getent groupadd id install ln mktemp od systemctl tr useradd usermod; do
  require_command "$command"
done
validate_prerequisites
ensure_service_account

install -d -o root -g root -m 0755 "$PREFIX" "$BIN_DIR" "$PREFIX/infra"
install -d -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0700 \
  "$DATA_DIR" \
  "$DATA_DIR/backups" \
  "$DATA_DIR/builds" \
  "$DATA_DIR/compose"
install -d -o root -g root -m 0755 "$CONFIG_DIR"
install -o root -g root -m 0755 "$SCRIPT_DIR/ignitify-core" "$CORE_BINARY"
install -o root -g root -m 0755 "$SCRIPT_DIR/railpack" "$BIN_DIR/railpack"
write_launcher
link_launcher_if_available

if [[ "$INSTALL_INGRESS_ASSETS" -eq 1 ]]; then
  install_ingress_assets
  info "installed Traefik operator assets"
fi

if [[ ! -e "$CONFIG_FILE" ]]; then
  write_default_config
  info "created $CONFIG_FILE with a generated bootstrap secret"
else
  warn "preserving existing configuration: $CONFIG_FILE"
fi
ensure_ingress_runtime_config

write_systemd_unit
systemctl daemon-reload
systemctl enable "$SERVICE_NAME"

if [[ "$START_SERVICE" -eq 1 ]]; then
  if systemctl is-active --quiet "$SERVICE_NAME"; then
    systemctl restart "$SERVICE_NAME"
  else
    systemctl start "$SERVICE_NAME"
  fi
  info "enabled and started $SERVICE_NAME"
else
  info "installed $SERVICE_NAME without starting it"
fi

info "Ignitify installed at $PREFIX"
info "retrieve the one-time bootstrap secret with: sudo awk -F= '/^IGNITIFY_BOOTSTRAP_SECRET=/{print \$2}' $CONFIG_FILE"
if [[ "$START_SERVICE" -eq 1 ]]; then
  info "open http://127.0.0.1:5656 and use the bootstrap secret to create the first platform operator"
else
  info "start Ignitify with: sudo systemctl start $SERVICE_NAME"
fi
info "for remote browser access, configure an HTTPS reverse proxy plus IGNITIFY_REMOTE_MODE, IGNITIFY_TRUST_PROXY_HEADERS, and IGNITIFY_TRUSTED_ORIGINS in $CONFIG_FILE"
info "configure the ACME contact email and domain policy in Infrastructure before enabling public service domains"
info "Docker group access is privileged; protect the host and restrict repository access accordingly."
