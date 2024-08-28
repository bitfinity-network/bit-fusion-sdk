#!/bin/bash

ordhook db sync --config-path=/Ordhook.toml
ordhook service start --post-to=http://hiro-ordinals-api:3099/payload --auth-token=1 --config-path=/Ordhook.toml
