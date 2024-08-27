#!/usr/bin/env bash

cd /app/ordinals-api
local-ssl-proxy --source 8005 --target 3000 --key localhost+3-key.pem --cert localhost+3.crt &

# postgres params
export PGHOST='postgres'
export PGUSER='postgres'
export PGDATABASE='postgres'
export PGPASSWORD='postgres'

npm run start
