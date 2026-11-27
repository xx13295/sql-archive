#!/usr/bin/env sh
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
ENGINE_DIR="${SQL_ARCHIVE_ENGINE_DIR:-$HERE/../../tool/java}"
ENGINE_JAR=$(ls "$ENGINE_DIR"/sql-translate*.jar | head -n 1)
CLASSES="$HERE/dist/classes"
rm -rf "$CLASSES"
mkdir -p "$CLASSES" "$HERE/dist"
find "$HERE/src" -name '*.java' > "$HERE/dist/sources.txt"
javac -encoding UTF-8 -cp "$ENGINE_JAR" -d "$CLASSES" @"$HERE/dist/sources.txt"
(cd "$CLASSES" && jar cf "$HERE/dist/sql-archive-bridge.jar" cn)
echo "built $HERE/dist/sql-archive-bridge.jar"