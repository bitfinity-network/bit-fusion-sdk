#!/usr/bin/env sh
VERSION="$1"
FILE="$2"

if [ -z "${GH_API_TOKEN}" ]; then
    echo "Required GH_API_TOKEN env variable with appropriate GitHub API token"
    exit 1
fi

REPO="bitfinity-network/evm-canister"
WASM_DIR=".artifact"

script_dir=$(dirname $0)
assets_dir=$(realpath "${script_dir}/../$WASM_DIR")

if [ -f "$assets_dir/$FILE" ]; then
    echo "File aready downloaded $assets_dir/$FILE"
    echo "Skip download and unpacking"
    exit 0
fi

set -e

mkdir -p "$assets_dir"

assets_file=$(mktemp /tmp/abc-script.XXXXXX)

curl -fL \
    -H "Accept: application/vnd.github+json" \
    -H "Authorization: Bearer $GH_API_TOKEN" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "https://api.github.com/repos/$REPO/releases/tags/$VERSION" \
    -o $assets_file


# echo "https://api.github.com/repos/$REPO/releases/tags/$VERSION"
# cat $assets

echo "Release assets downloaded"

# cat $assets_file | tr $'\u000a' ' ' | jq -c '.assets[]'
# cat $assets_file | jq -c '.assets[]'

# asset_id=$(cat $assets_file | tr $'\u000a' ' ' | jq -r ".assets[] | select(.name == \"${FILE}\").id")
asset_id=$(cat $assets_file | jq -r ".assets[] | select(.name == \"${FILE}\").id")

rm $assets_file

if [ -z "$asset_id" ]; then
    echo "Asset ID was not found for $FILE"
    exit 22
fi

echo "https://api.github.com/repos/$REPO/releases/assets/$asset_id"

curl -L \
    -H "Accept:  application/octet-stream" \
    -H "Authorization: Bearer $GH_API_TOKEN" \
    -H "X-GitHub-Api-Version: 2022-11-28" \
    "https://api.github.com/repos/$REPO/releases/assets/$asset_id" \
    -o "$assets_dir/$FILE"


if [ ! -f "$assets_dir/$FILE" ]; then
    echo "Release asset not found: $FILE"
    exit 33
fi

echo "Release file downloaded (^_^): $FILE"

cd $assets_dir
tar -xzf "$FILE"

echo "Unpaked to $assets_dir"
ls -l $assets_dir
