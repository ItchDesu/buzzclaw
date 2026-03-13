#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$ROOT_DIR/android_app/app/src/main/jniLibs"
APP_DIR="$ROOT_DIR/android_app"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found" >&2
  exit 1
fi

if ! cargo ndk --help >/dev/null 2>&1; then
  echo "cargo-ndk not found. Install with: cargo install cargo-ndk" >&2
  exit 1
fi

find_ndk_from_local_properties() {
  local props="$APP_DIR/local.properties"
  if [[ -f "$props" ]]; then
    local sdk
    sdk=$(grep -E '^sdk.dir=' "$props" | head -n1 | cut -d= -f2-)
    if [[ -n "$sdk" ]]; then
      sdk=$(echo "$sdk" | sed 's#\\#/#g')
      if [[ -d "$sdk/ndk" ]]; then
        local latest
        latest=$(ls -1 "$sdk/ndk" 2>/dev/null | sort -V | tail -n1)
        if [[ -n "$latest" && -d "$sdk/ndk/$latest" ]]; then
          echo "$sdk/ndk/$latest"
          return 0
        fi
      fi
      if [[ -d "$sdk/ndk-bundle" ]]; then
        echo "$sdk/ndk-bundle"
        return 0
      fi
    fi
  fi
  return 1
}

find_ndk_from_env() {
  if [[ -n "${ANDROID_NDK_HOME:-}" && -d "$ANDROID_NDK_HOME" ]]; then
    echo "$ANDROID_NDK_HOME"
    return 0
  fi
  if [[ -n "${ANDROID_NDK_ROOT:-}" && -d "$ANDROID_NDK_ROOT" ]]; then
    echo "$ANDROID_NDK_ROOT"
    return 0
  fi
  return 1
}

find_ndk_from_common_paths() {
  local candidates=(
    "$HOME/Android/Sdk/ndk"
    "$HOME/Android/Sdk/ndk-bundle"
    "$HOME/Android/sdk/ndk"
    "$HOME/Android/sdk/ndk-bundle"
  )

  for base in "${candidates[@]}"; do
    if [[ -d "$base" ]]; then
      if [[ "$base" == */ndk ]]; then
        local latest
        latest=$(ls -1 "$base" 2>/dev/null | sort -V | tail -n1)
        if [[ -n "$latest" && -d "$base/$latest" ]]; then
          echo "$base/$latest"
          return 0
        fi
      else
        echo "$base"
        return 0
      fi
    fi
  done
  return 1
}

NDK_PATH=""
if NDK_PATH=$(find_ndk_from_env); then
  :
elif NDK_PATH=$(find_ndk_from_local_properties); then
  :
elif NDK_PATH=$(find_ndk_from_common_paths); then
  :
else
  echo "NDK not found." >&2
  echo "Set ANDROID_NDK_HOME or ANDROID_NDK_ROOT, or add sdk.dir to android_app/local.properties." >&2
  exit 1
fi

export ANDROID_NDK_HOME="$NDK_PATH"

mkdir -p "$OUT_DIR"

TARGETS=(
  "arm64-v8a"
  "armeabi-v7a"
  "x86"
  "x86_64"
)

cd "$ROOT_DIR"

cargo ndk \
  $(printf -- '-t %s ' "${TARGETS[@]}") \
  -o "$OUT_DIR" \
  build --release --features jni --no-default-features

echo "Built libbuzzclaw.so into $OUT_DIR/<abi>/libbuzzclaw.so"
