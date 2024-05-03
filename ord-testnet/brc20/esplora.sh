#!/usr/bin/env bash

local-ssl-proxy --source 5001 --target 5000 --key localhost+3-key.pem --cert localhost+3.pem &
npm run dev-server
