#!/usr/bin/env bash
# --------------------------------------------------------------------
# SCRIPT: temp.sh
# DESCRIPTION: Native server temperature and conditional status script
# --------------------------------------------------------------------

MODE="C"
SHOW_LABELS="false"

# Parse CLI arguments
for arg in "$@"; do
    case "$arg" in
        --fahrenheit|-f)
            MODE="F"
            ;;
        --celsius|-c)
            MODE="C"
            ;;
        --labels|-l)
            SHOW_LABELS="true"
            ;;
        --help|-h)
            echo "Usage: $(basename "$0") [--fahrenheit|-f] [--celsius|-c] [--labels|-l]"
            exit 0
            ;;
    esac
done

read_thermal() {
    local temp=""
    local type=""

    # Loop through native thermal zones to find the main CPU package
    for zone in /sys/class/thermal/thermal_zone*; do
        if [ -f "$zone/type" ]; then
            type=$(cat "$zone/type" 2>/dev/null)
            if [ "$type" = "x86_pkg_temp" ] || [ "$type" = "cpu-thermal" ]; then
                temp=$(cat "$zone/temp" 2>/dev/null)
                break
            fi
        fi
    done

    # Fallback: If no explicit CPU zone type is found, use the first zone safely
    if [ -z "$temp" ] && [ -f /sys/class/thermal/thermal_zone0/temp ]; then
        temp=$(cat /sys/class/thermal/thermal_zone0/temp 2>/dev/null)
    fi

    echo "$temp"
}

# Enforce 3-second timeout.
# When timeout fires (or read_thermal exits non-zero), print -1 and exit with code 1.
if command -v timeout >/dev/null 2>&1; then
    TEMP=$(timeout 3s bash -c "$(declare -f read_thermal); read_thermal")
    TIMEOUT_STATUS=$?
    if [ "$TIMEOUT_STATUS" -ne 0 ] || [ -z "$TEMP" ]; then
        echo "-1"
        exit 1
    fi
else
    # Fallback if timeout utility is not present: run read_thermal directly
    TEMP=$(read_thermal)
    if [ -z "$TEMP" ]; then
        echo "-1"
        exit 1
    fi
fi

# Convert micro-degrees to standard degrees Celsius
TEMP=$((TEMP / 1000))

# Determine the status based on Celsius thresholds
if [ "$TEMP" -le 45 ]; then
    STATUS="idle"
elif [ "$TEMP" -le 75 ]; then
    STATUS="load"
elif [ "$TEMP" -le 85 ]; then
    STATUS="hot"
else
    STATUS="overheating"
fi

# Convert to degrees F
TEMP_F=$((TEMP * 9 / 5 + 32))

# Format the base temperature string
if [ "$MODE" = "F" ]; then
    OUTPUT="${TEMP_F}°F"
else
    OUTPUT="${TEMP}°C"
fi

# Append the status label only if requested
if [ "$SHOW_LABELS" = "true" ]; then
    echo "${OUTPUT} - Status: ${STATUS}"
else
    echo "${OUTPUT}"
fi
