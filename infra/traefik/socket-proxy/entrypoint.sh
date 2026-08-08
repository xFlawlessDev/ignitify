#!/bin/sh
set -eu

umask 077
disable_ipv6_lower=$(printf '%s' "${DISABLE_IPV6:-false}" | tr '[:upper:]' '[:lower:]')
case "$disable_ipv6_lower" in
  1|true|yes) bind_config=':2375' ;;
  *) bind_config='[::]:2375 v4v6' ;;
esac

sed "s/\${BIND_CONFIG}/$bind_config/g" \
  /usr/local/etc/haproxy/haproxy.cfg.template > /tmp/haproxy.cfg
exec "$@"
