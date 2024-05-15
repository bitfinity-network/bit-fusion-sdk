#!/usr/bin/env bash

local-ssl-proxy --source 9001 --target 3001 --key localhost+3-key.pem --cert localhost+3.pem &
npm start
