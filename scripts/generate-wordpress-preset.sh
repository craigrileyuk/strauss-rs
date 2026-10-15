#!/bin/sh
# Regenerates src/symbols/presets/wordpress.txt from php-stubs/wordpress-stubs,
# using strauss-rs itself to list the symbols the stubs declare.
#
#   scripts/generate-wordpress-preset.sh [stubs-version]
#
# Needs composer, jq and a built strauss-rs (cargo build --release). Only
# symbols in the global namespace are kept: a namespaced one, such as
# `WpOrg\Requests\Client`, is renamed like any other.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
binary=${STRAUSS_RS_BIN:-$root/target/release/strauss-rs}
version=${1:-*}
work=$(mktemp -d "${TMPDIR:-$HOME}/strauss-rs-wp-stubs.XXXXXX")
trap 'rm -rf "$work"' EXIT

(
    cd "$work"
    printf '{"name":"strauss-rs/wordpress-preset","require":{}}\n' > composer.json
    composer require "php-stubs/wordpress-stubs:$version" \
        --no-interaction --no-scripts --no-plugins --quiet
)

stubs=$(jq -r '.packages[] | select(.name == "php-stubs/wordpress-stubs") | .version' \
    "$work/vendor/composer/installed.json")

"$binary" symbols -C "$work" -q > "$work/symbols.json"

{
    echo "# Wordpress core's global classes, interfaces, traits, functions and constants."
    echo "#"
    echo "# Symbols with these names are never renamed when the \`wordpress\` preset is on"
    echo "# (\`exclude_presets\`): a library's shim for them would no longer be what"
    echo "# Wordpress expects."
    echo "#"
    echo "# Generated from php-stubs/wordpress-stubs $stubs by"
    echo "# scripts/generate-wordpress-preset.sh. Don't edit by hand, except for the"
    echo "# constants, which the stubs don't declare: those come from"
    echo "# scripts/wordpress-constants.txt."
    echo
    jq -r '.packages["php-stubs/wordpress-stubs"]
        | (.classes + .interfaces + .traits)[] | select(contains("\\") | not) | "c " + .' \
        "$work/symbols.json" | LC_ALL=C sort -u
    jq -r '.packages["php-stubs/wordpress-stubs"]
        | .functions[] | select(contains("\\") | not) | "f " + .' \
        "$work/symbols.json" | LC_ALL=C sort -u
    grep -v '^#' "$root/scripts/wordpress-constants.txt" | grep -v '^$' | sed 's/^/k /' | LC_ALL=C sort -u
} > "$root/src/symbols/presets/wordpress.txt"

echo "wrote src/symbols/presets/wordpress.txt from wordpress-stubs $stubs"
