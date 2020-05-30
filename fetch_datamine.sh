#!/usr/bin/env bash

set -e

source .env

if [ -z "$API_KEY" ]; then
  echo "API_KEY must be set in .env"
  exit 1
fi

echo
echo '#############################'
echo '### This can take a while ###'
echo '#############################'
echo

SHEET_ID='13d_LAJPlxMa_DubPTuirkIV4DERBMXbrWQsmSh8ReK4'

curl \
  "https://sheets.googleapis.com/v4/spreadsheets/$SHEET_ID?includeGridData=true&key=$API_KEY" \
  --header 'Accept: application/json' \
  --compressed > datamine.json
