#!/usr/bin/env bash

local-ssl-proxy --source 8005 --target 3000 --key localhost+3-key.pem --cert localhost+3.crt &

cd /app/ordinals-api-regtest/
# postgres params
export PGHOST='postgres'
export PGUSER='postgres'
export PGDATABASE='postgres'
export PGPASSWORD='postgres'
export ORDHOOK_NODE_RPC_HOST='ordhook'
export CHAINHOOK_NODE_AUTH_TOKEN="1"
export CHAINHOOK_AUTO_PREDICATE_REGISTRATION="false"
export INSCRIPTION_GAP_DETECTION_ENABLED="false"

npm run start
