#!/bin/bash

ls -l
ls -l ordhook
rm -rf ordhook/*

ordhook db sync --config-path=/Ordhook.toml
ordhook service start --post-to=http://hiro-ordinals-api:3099/payload --auth-token=1 --config-path=/Ordhook.toml
