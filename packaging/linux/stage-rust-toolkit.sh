#!/bin/bash
# Stage the native toolkit for embedding; run in the Debian 11 builder.
set -euo pipefail
stage=${1:?Usage: stage-rust-toolkit.sh DESTINATION [PREFIX]}
prefix=${2:-/opt/tk92}
mkdir -p "$stage/lib" "$stage/licenses"
list=$(mktemp)
trap 'rm -f "$list"' EXIT
for library in "$prefix/lib/libtcl9.1.so" "$prefix/lib/libtcl9tk9.2.so"; do
    printf '%s\n' "$library" >> "$list"
    dependencies=$(ldd "$library")
    if [[ "$dependencies" == *'not found'* ]]; then
        printf '%s\n' "$dependencies" >&2
        exit 1
    fi
    awk '/=> \// {print $3}' <<< "$dependencies" >> "$list"
done
sort -u "$list" -o "$list"
: > "$stage/libraries.sha256"
while IFS= read -r library; do
    name=${library##*/}
    case "$name" in
        libc.so.6|libm.so.6|libdl.so.2|librt.so.1|libpthread.so.0|libresolv.so.2|libutil.so.1|ld-linux-x86-64.so.2) continue ;;
    esac
    cp -L "$library" "$stage/lib/$name"
    (cd "$stage/lib" && sha256sum "$name") >> "$stage/libraries.sha256"
    while IFS= read -r owner; do
        package=${owner%%:*}
        notice=/usr/share/doc/$package/copyright
        if [[ -f "$notice" ]]; then cp "$notice" "$stage/licenses/$package.copyright"; fi
    done < <(dpkg-query -S "$library" 2>/dev/null || true)
done < "$list"
shopt -s nullglob
for base in /tmp/bb-deps /tmp/tk92; do
    for project in "$base"/*/; do
        name=$(basename "$project")
        for notice in "$project"/LICENSE* "$project"/COPYING* "$project"/license.terms; do
            if [[ -f "$notice" ]]; then cp "$notice" "$stage/licenses/$name-$(basename "$notice")"; fi
        done
    done
done
if [[ -d /opt/bb-deps/share/licenses ]]; then
    cp -a /opt/bb-deps/share/licenses/. "$stage/licenses/"
fi
for name in tcl9.1 tk9.2; do
    mkdir -p "$stage/$name"
    cp -a "$prefix/lib/$name/." "$stage/$name/"
done
printf 'Staged %s native libraries\n' "$(wc -l < "$stage/libraries.sha256")"
