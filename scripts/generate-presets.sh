#!/bin/sh
# Regenerates src/symbols/presets/<preset>.txt from the preset's php-stubs
# package, using strauss-rs itself to list the symbols the stubs declare.
#
#   scripts/generate-presets.sh [preset...] [--version=VERSION]
#
# With no preset, all of them. VERSION is the stubs package's version, for one
# preset at a time; the default is the latest.
#
# Needs composer, jq and a built strauss-rs (cargo build --release). Only
# symbols in the global namespace are kept: a namespaced one, such as
# `WpOrg\Requests\Client`, is renamed like any other.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
binary=${STRAUSS_RS_BIN:-$root/target/release/strauss-rs}
version='*'
presets=

for argument in "$@"; do
    case $argument in
        --version=*) version=${argument#--version=} ;;
        *) presets="$presets $argument" ;;
    esac
done

[ -n "$presets" ] || presets="wordpress woocommerce"

# stubs_package, description
describe() {
    case $1 in
        wordpress)
            stubs=php-stubs/wordpress-stubs
            title="Wordpress core's"
            ;;
        woocommerce)
            stubs=php-stubs/woocommerce-stubs
            title="WooCommerce's"
            ;;
        *)
            echo "unknown preset: $1" >&2
            exit 1
            ;;
    esac
}

generate() { # preset
    preset=$1
    describe "$preset"

    work=$(mktemp -d "${TMPDIR:-$HOME}/strauss-rs-$preset-stubs.XXXXXX")
    trap 'rm -rf "$work"' EXIT

    (
        cd "$work"
        printf '{"name":"strauss-rs/%s-preset","require":{}}\n' "$preset" > composer.json
        composer require "$stubs:$version" \
            --no-interaction --no-scripts --no-plugins --quiet
    )

    installed=$(jq -r --arg name "$stubs" \
        '.packages[] | select(.name == $name) | .version' \
        "$work/vendor/composer/installed.json")

    # With presets on, the symbols being listed would be left out of the list.
    printf '{"exclude_presets": []}\n' > "$work/strauss.json"

    "$binary" symbols -C "$work" -q > "$work/symbols.json"

    {
        echo "# $title global classes, interfaces, traits, functions and constants."
        echo "#"
        echo "# Symbols with these names are never renamed when the \`$preset\` preset is on"
        echo "# (\`exclude_presets\`): a library's copy of them would no longer be what"
        echo "# the rest of the platform expects."
        echo "#"
        echo "# Generated from $stubs $installed by"
        echo "# scripts/generate-presets.sh. Don't edit by hand, except for the constants,"
        echo "# which the stubs mostly don't declare: those come from"
        echo "# scripts/$preset-constants.txt."
        echo
        jq -r --arg name "$stubs" '.packages[$name]
            | (.classes + .interfaces + .traits)[] | select(contains("\\") | not) | "c " + .' \
            "$work/symbols.json" | LC_ALL=C sort -u
        jq -r --arg name "$stubs" '.packages[$name]
            | .functions[] | select(contains("\\") | not) | "f " + .' \
            "$work/symbols.json" | LC_ALL=C sort -u
        {
            jq -r --arg name "$stubs" '.packages[$name]
                | .constants[] | select(contains("\\") | not)' "$work/symbols.json"
            grep -v '^#' "$root/scripts/$preset-constants.txt" | grep -v '^$'
        } | sed 's/^/k /' | LC_ALL=C sort -u
    } > "$root/src/symbols/presets/$preset.txt"

    echo "wrote src/symbols/presets/$preset.txt from $stubs $installed"

    rm -rf "$work"
    trap - EXIT
}

for preset in $presets; do
    generate "$preset"
done
