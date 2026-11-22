#!/usr/bin/env bash
set -euo pipefail

# Terminal Weather Radar Viewer (NWS + chafa)
# Resolves a 5-digit US ZIP code, 3- or 4-letter airport code, or direct NEXRAD radar ID
# and renders either a static image or an animated radar loop in ANSI/Braille characters.

USER_AGENT="terminal-radar-cli/1.0"
ANIMATE=0
SYMBOLS="block"
COLORS="full"
QUERY=""

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS] <ZIP | AIRPORT | RADAR_ID>

Examples:
  $(basename "$0") 48201              # Static radar for ZIP 48201
  $(basename "$0") -a DTW             # Animated radar loop near DTW airport
  $(basename "$0") -a -b KGRR         # Animated radar in Unicode Braille for KGRR
  $(basename "$0") CENTGRLAKES        # Static Central Great Lakes regional mosaic

Options:
  -a, --animate       Play the animated radar loop (press Ctrl+C to exit)
  -b, --braille       Use Unicode Braille characters instead of ANSI half-blocks
  -h, --help          Show this help message
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -a|--animate)
            ANIMATE=1
            shift
            ;;
        -b|--braille)
            SYMBOLS="braille"
            COLORS="256"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        -*)
            echo "Unknown option: $1" >&2
            usage
            exit 1
            ;;
        *)
            QUERY="$1"
            shift
            ;;
    esac
done

if [[ -z "$QUERY" ]]; then
    usage
    exit 1
fi

for cmd in curl jq chafa; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: Required tool '$cmd' is not installed." >&2
        exit 1
    fi
done

resolve_radar_id() {
    local input="${1^^}"

    # 1. 5-digit US ZIP code -> lat/lon -> NWS radarStation
    if [[ "$input" =~ ^[0-9]{5}$ ]]; then
        local coords
        coords=$(curl -fsSL "https://api.zippopotam.us/us/${input}" 2>/dev/null             | jq -r '.places[0] | "\(.latitude),\(.longitude)"' 2>/dev/null || true)
        if [[ -z "$coords" || "$coords" == "null,null" ]]; then
            echo "Error: Could not find coordinates for ZIP code $input." >&2
            return 1
        fi
        curl -fsSL -H "User-Agent: $USER_AGENT" "https://api.weather.gov/points/${coords}" 2>/dev/null             | jq -r '.properties.radarStation // empty'
        return 0
    fi

    # 2. 3-letter IATA airport code -> prepend 'K' for contiguous US ICAO lookup
    if [[ "$input" =~ ^[A-Z]{3}$ ]]; then
        input="K${input}"
    fi

    # 3. 4-letter station code: try NWS observation station lookup first (e.g., KDTW -> KDTX)
    if [[ "$input" =~ ^[A-Z]{4}$ ]]; then
        local station_radar
        station_radar=$(curl -fsSL -H "User-Agent: $USER_AGENT" "https://api.weather.gov/stations/${input}" 2>/dev/null             | jq -r '.properties.radarStation // empty' 2>/dev/null || true)
        if [[ -n "$station_radar" ]]; then
            echo "$station_radar"
            return 0
        fi
        # Otherwise assume the user passed a direct NEXRAD radar ID (e.g., KDTX, KGRR)
        echo "$input"
        return 0
    fi

    # 4. Regional mosaic names (e.g., CENTGRLAKES, CONUS, NORTHEAST)
    echo "$input"
}

RADAR_ID=$(resolve_radar_id "$QUERY")

if [[ -z "$RADAR_ID" ]]; then
    echo "Error: Could not resolve a NEXRAD radar station for '$QUERY'." >&2
    exit 1
fi

if [[ "$ANIMATE" -eq 1 ]]; then
    SUFFIX="_loop.gif"
    MODE_LABEL="animated loop (press Ctrl+C to quit)"
else
    SUFFIX="_0.gif"
    MODE_LABEL="latest frame"
fi

RADAR_URL="https://radar.weather.gov/ridge/standard/${RADAR_ID}${SUFFIX}"

echo "Fetching ${MODE_LABEL} for radar station: ${RADAR_ID}..." >&2

if ! curl -fsSL -H "User-Agent: $USER_AGENT" "$RADAR_URL"     | chafa --symbols "$SYMBOLS" --colors "$COLORS" --animate on; then
    echo "Error: Failed to fetch or render radar from $RADAR_URL" >&2
    exit 1
fi
