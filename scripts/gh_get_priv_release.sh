#!/usr/bin/env bash
# Usage: GH_API_TOKEN="TOKEN" gh_get_priv_release.sh TARGET_DIR OWNER REPO RELEASE_TAG ASSET1_NAME [ASSET2_NAME ...]

# Check dependencies.
set -e
type curl grep sed tr >&2

# Validate settings.
[ "$GH_API_TOKEN" ] || { echo "Error: Please define GH_API_TOKEN variable." >&2; exit 1; }
[ $# -lt 5 ] && { echo "Usage: $0 TARGET_DIR OWNER REPO RELEASE_TAG ASSET1_NAME [ASSET2_NAME ...]"; exit 1; }
[ "$TRACE" ] && set -x

read target_dir owner repo tag <<<$@
shift 4

# Define variables.
GH_API="https://api.github.com"
GH_REPO="$GH_API/repos/$owner/$repo"
GH_TAGS="$GH_REPO/releases/tags/$tag"
AUTH="Authorization: token $GH_API_TOKEN"
WGET_ARGS="--content-disposition --auth-no-challenge --no-cookie"
CURL_ARGS="-LJO#"

# Validate token.
curl -o /dev/null -sH "$AUTH" $GH_REPO || { echo "Error: Invalid repo, token or network issue!";  exit 1; }


# Read asset tags.
set +e
response=$(curl -sH "$AUTH" $GH_TAGS)
R="$?"
# Sometimes on success it could return 6
if [ "$R" != "0" ] && [ "$R" != "6" ]; then
    echo "Error: Can not get release info. Response code: $R"
    echo "$response"
    exit 1
fi

set -e
# Download assets
for name in "$@"; do
cd $target_dir
echo "DOWNLOADING $name"
    # Get ID of the asset based on given name.
    eval $(echo "$response" | grep -C3 "name.:.\+$name" | grep -w id | tr : = | tr -cd '[[:alnum:]]=')
    #id=$(echo "$response" | jq --arg name "$name" '.assets[] | select(.name == $name).id') # If jq is installed, this can be used instead. 
    
    [ "$id" ] || { echo "Error: Failed to get asset id for \"$name\", response: $response" | awk 'length($0)<100' >&2; exit 1; }

    GH_ASSET="$GH_REPO/releases/assets/$id"

    # Download asset file.
    echo "Downloading asset \"$name\" ..." >&2
    rm -f "$name"
    curl $CURL_ARGS -H "Authorization: token $GH_API_TOKEN" -H 'Accept: application/octet-stream' "$GH_ASSET"

    case $name in
        *.tar.gz)
            # Unpack archive and remove it.
            echo "Unpacking \"$name\""
            tar -xvf $name
            rm -f $name
            ;;
    esac
done

echo "$0 is done \(^_^)/" >&2
