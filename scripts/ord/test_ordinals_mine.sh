#!/usr/bin/env sh

# Generates bitcoins for specific address(mining).

if [ -z "$1" ] ; then
    echo "Please, provide blocks amount (e.g 101)"
    exit 1
fi

if [ -z "$2" ] ; then
    echo "Please, provide valid address"
    exit 1
fi

docker exec bitcoind bitcoin-cli -regtest generatetoaddress $1 $2
