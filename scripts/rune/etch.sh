# Sets up the docker bitcoin deployment and creates a new test rune SUPERMAXRUNENAME
#
# This script is supposed to be run against a fresh new docker test environment. Recreate the docker volumes before
# rerunning it:
#
# cd btc-deploy
# docker compose down -v
# docker compose up
#
# After the script execution finishes there will be:
# * admin btc wallet with a lot of BTC on it
# * SUPERMAXRUNENAME rune etched
# * ord wallet with SUPERMAXRUNENAME balance ready to be used

set -e

ORD_DATA=$PWD/target/ord
rm -rf $ORD_DATA
mkdir -p $ORD_DATA

echo "Ord data folder: $ORD_DATA"

if ! command -v bitcoin-cli &> /dev/null
then
  if command -v bitcoin-core.cli &> /dev/null
  then
    bc="bitcoin-core.cli -conf=$PWD/btc-deploy/bitcoin.conf"
    echo "Using bitcoin-core.cli as bitcoin-cli"
  else
    echo "bitcoin-cli could not be found"
    exit 1
  fi
else
  bc="bitcoin-cli -conf=$PWD/btc-deploy/bitcoin.conf"
fi

ordw="ord -r --data-dir $ORD_DATA --index-runes wallet --server-url http://localhost:8000"

if [[ $($bc listwallets | jq .[0]) = \"admin\" ]]
then
  echo "Wallet admin already exists"
else
  echo "Creating a new admin wallet"
  $bc createwallet admin &> /dev/null
fi

ADMIN_ADDRESS=$($bc -rpcwallet=admin getnewaddress)

# Ensure we have some BTC to spend
$bc generatetoaddress 101 $ADMIN_ADDRESS &> /dev/null
echo "Generated 101 blocks for admin"

export ORD_BITCOIN_RPC_USERNAME=ic-btc-integration
export ORD_BITCOIN_RPC_PASSWORD="QPQiNaph19FqUsCrBRN0FII7lyM26B51fAMeBQzCb-E="


$ordw create &> /dev/null
sleep 3
echo "Created ord wallet"

ORD_ADDRESS=$($ordw receive | jq -r .addresses[0])

$bc -rpcwallet=admin sendtoaddress $ORD_ADDRESS 10 &> /dev/null
$bc -rpcwallet=admin generatetoaddress 1 $ADMIN_ADDRESS &> /dev/null

echo "Sent 10 BTC to ord wallet address $ORD_ADDRESS"

sleep 5
$ordw batch --fee-rate 10 --batch ./scripts/rune/batch.yaml &

sleep 1
$bc -rpcwallet=admin generatetoaddress 10 $ADMIN_ADDRESS

sleep 5
$bc -rpcwallet=admin generatetoaddress 1 $ADMIN_ADDRESS

sleep 3
$ordw balance

ID=$(ord -r --data-dir $ORD_DATA --index-runes runes | jq -r .runes.SUPERMAXRUNENAME.id)

echo
echo "CONGRATULATIONS"
echo "Rune SUPERMAXRUNENAME is etched with id: $ID"