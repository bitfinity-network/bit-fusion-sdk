#!/bin/bash

ordhook db new --config-path=/Ordhook.toml
echo "Database created"
ordhook service start --post-to=http://ordinals-api:3099/payload --auth-token=1 --config-path=/Ordhook.toml
