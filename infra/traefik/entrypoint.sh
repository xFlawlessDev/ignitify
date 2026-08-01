#!/bin/sh
set -eu

umask 077
: > /letsencrypt/acme.json
chmod 0600 /letsencrypt/acme.json
exec /entrypoint.sh "$@"
