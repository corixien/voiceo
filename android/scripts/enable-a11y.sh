#!/usr/bin/env bash
# Enables an accessibility service of a sideloaded app over adb, without the Play Store:
#  1. lifts Android's "restricted settings" block for the package
#  2. appends the service to the enabled accessibility services (keeps the existing ones)
# Usage: enable-a11y.sh <package> <service-class> [adb-serial]
# Example: enable-a11y.sh dev.voiceo.app dev.voiceo.app.VoiceoAccessibilityService
set -euo pipefail
pkg="${1:?package}"; svc="${2:?service class}"; serial="${3:-}"
adb_() { if [ -n "$serial" ]; then adb -s "$serial" "$@"; else adb "$@"; fi; }
comp="$pkg/$svc"
adb_ shell cmd appops set "$pkg" ACCESS_RESTRICTED_SETTINGS allow
cur="$(adb_ shell settings get secure enabled_accessibility_services | tr -d '\r')"
case ":$cur:" in
  *":$comp:"*) echo "already enabled" ;;
  *) if [ -z "$cur" ] || [ "$cur" = "null" ]; then new="$comp"; else new="$cur:$comp"; fi
     adb_ shell settings put secure enabled_accessibility_services "$new"
     adb_ shell settings put secure accessibility_enabled 1
     echo "enabled $comp" ;;
esac
