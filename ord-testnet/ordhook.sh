#!/usr/bin/env bash

local-ssl-proxy --source 9001 --target 20456 --key localhost+3-key.pem --cert localhost+3.pem &
ordhook service start --post-to=http://ordinals-api:3099/payload --auth-token=1 --config-path=./Ordhook.toml
