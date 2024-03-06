#!/usr/bin/env sh

git clone https://github.com/hirosystems/ordinals-api
python3 scripts/patch_ordinals_api.py ordinals-api/src/pg/brc20/brc20-pg-store.ts
cd ordinals-api && docker build --tag ordinals-api .
rm -rf ordinals-api
