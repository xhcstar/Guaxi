#!/usr/bin/env bash
set -euo pipefail

BASE_URL="https://mirrors.cloud.tencent.com/gradle"
GRADLE_VERSION="8.14.3"
PROJECT_ROOT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --base-url)
      BASE_URL="${2:-}"
      shift 2
      ;;
    --gradle-version)
      GRADLE_VERSION="${2:-}"
      shift 2
      ;;
    --project-root)
      PROJECT_ROOT="${2:-}"
      shift 2
      ;;
    *)
      echo "[capture-android] unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -z "$PROJECT_ROOT" ]]; then
  PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
fi

WRAPPER_PROPERTIES="$PROJECT_ROOT/src-tauri/gen/android/gradle/wrapper/gradle-wrapper.properties"

if [[ ! -f "$WRAPPER_PROPERTIES" ]]; then
  DISCOVERED_WRAPPER="$(find "$PROJECT_ROOT/src-tauri/gen" -path '*/gradle/wrapper/gradle-wrapper.properties' -print -quit 2>/dev/null || true)"
  if [[ -n "$DISCOVERED_WRAPPER" ]]; then
    WRAPPER_PROPERTIES="$DISCOVERED_WRAPPER"
    echo "[capture-android] using discovered wrapper: $WRAPPER_PROPERTIES"
  fi
fi

if [[ ! -f "$WRAPPER_PROPERTIES" ]]; then
  echo "[capture-android] gradle-wrapper.properties not found: $WRAPPER_PROPERTIES" >&2
  echo "[capture-android] Run npm run android:init first, then rerun this script." >&2
  exit 1
fi

DISTRIBUTION_LINE="$(grep -E '^distributionUrl=' "$WRAPPER_PROPERTIES" | head -n 1 || true)"
if [[ -n "$DISTRIBUTION_LINE" ]]; then
  FILE_NAME="$(basename "${DISTRIBUTION_LINE#distributionUrl=}")"
else
  echo "[capture-android] distributionUrl not found, repairing with Gradle $GRADLE_VERSION"
  FILE_NAME="gradle-${GRADLE_VERSION}-bin.zip"
fi

if [[ ! "$FILE_NAME" =~ ^gradle-.+\.zip$ ]]; then
  echo "[capture-android] cannot parse Gradle distribution file: $FILE_NAME" >&2
  exit 1
fi

BASE_URL="${BASE_URL%/}"
NEW_URL="$BASE_URL/$FILE_NAME"
ESCAPED_URL="${NEW_URL/https:/https\\:}"
ESCAPED_URL="${ESCAPED_URL/http:/http\\:}"

# Design note:
# Tauri Android uses Gradle Wrapper. In China, services.gradle.org often times out,
# so this script rebuilds the small Java properties file and points distributionUrl
# to a mirror. The file body is ASCII only to avoid UTF-8 BOM/newline issues.
cat > "$WRAPPER_PROPERTIES" <<EOF
distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=$ESCAPED_URL
networkTimeout=10000
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
EOF

echo "[capture-android] Gradle wrapper distributionUrl updated:"
echo "[capture-android] $NEW_URL"
