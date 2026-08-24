#!/bin/sh
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
printf 'this hook must never run\n' > "$script_dir/../hook-ran.txt"
