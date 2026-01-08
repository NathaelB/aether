#!/bin/sh

set -e

rm -rf /usr/share/nginx/html/*
cp -r /usr/local/src/aether/* /usr/share/nginx/html
envsubst < /usr/local/src/aether/config.json > /usr/share/nginx/html/config.json
