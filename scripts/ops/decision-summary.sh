#!/usr/bin/env sh
set -eu

: "${OPENCADE_API_URL:?set OPENCADE_API_URL to the server origin}"
: "${SESSION_TOKEN:?set SESSION_TOKEN to an authenticated operator session}"
: "${OPERATOR_TOKEN:?set OPERATOR_TOKEN to the independent operator credential}"

fetch() {
  curl --fail --silent --show-error \
    --header "Authorization: Bearer $SESSION_TOKEN" \
    --header "x-operator-token: $OPERATOR_TOKEN" \
    "$OPENCADE_API_URL$1"
}

echo "campaign"
fetch "/api/v1/alpha/campaign"
echo
echo "activation"
fetch "/api/v1/telemetry/activation"
echo
