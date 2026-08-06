#!/bin/sh
set -eu

umask 077
if [ ! -e /letsencrypt/acme.json ]; then
  : > /letsencrypt/acme.json
fi
chmod 0600 /letsencrypt/acme.json
exec /entrypoint.sh "$@"
