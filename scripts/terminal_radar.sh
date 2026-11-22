#!/usr/bin/env bash
# terminal_radar.sh - Pure Bash interactive animated NWS weather radar viewer
# Zero Python or external image-library dependencies (uses bash, curl, chafa).

set -euo pipefail

# --- Configuration ---
DEFAULT_LOCATION="KDTX"
SYMBOL_MODE="block"      # "block" (ANSI half-blocks) or "braille" (Unicode Braille)
COLOR_MODE="full"        # "full" (truecolor) or "256"
SIZE=""                  # Optional WxH (e.g. 80x40), auto-detected if empty
DELAY_MS=250             # Initial frame delay in milliseconds
MIN_DELAY_MS=50
MAX_DELAY_MS=2000
STEP_DELAY_MS=50
USER_AGENT="terminal-radar-bash/3.0"

TMP_DIR=""
FRAME_COUNT=0
LAST_MOD_EPOCH=0
STATION=""
RESOLVED_LABEL=""
FRAMES=()
TIMESTAMPS=()

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS] [LOCATION]

Render an interactive, looping NWS weather radar in your terminal using pure Bash.
No Python or extra image libraries required—only curl and chafa.

LOCATION can be:
  - A 5-digit US ZIP code (e.g., 48201)
  - A 3- or 4-letter US airport code (e.g., DTW, KDTW, ORD)
  - A 4-letter NEXRAD radar station ID (e.g., KDTX, KGRR)
  - An NWS regional mosaic name (e.g., CENTGRLAKES, CONUS)

Options:
  -b, --braille       Use Unicode Braille characters instead of ANSI half-blocks
  -s, --size WxH      Set output dimensions in columns x rows (e.g., 80x40)
  -d, --delay MS      Initial delay between frames in milliseconds (default: 250)
  -h, --help          Show this help message

Interactive Controls:
  + / =               Speed up animation
  - / _               Slow down animation
  r                   Refresh radar frames from NWS
  Space               Pause / resume playback
  q                   Quit
EOF
    exit 0
}

cleanup() {
    printf "\033[?25h\033[0m"
    stty echo icanon 2>/dev/null || true
    if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT INT TERM

# --- Parse Arguments ---
TARGET=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -b|--braille)
            SYMBOL_MODE="braille"
            shift
            ;;
        -s|--size)
            SIZE="$2"
            shift 2
            ;;
        -d|--delay)
            DELAY_MS="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        -*)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
        *)
            TARGET="$1"
            shift
            ;;
    esac
done

TARGET="${TARGET:-$DEFAULT_LOCATION}"
TARGET="${TARGET^^}"

# --- Dependency Check (Pure Bash + curl + chafa) ---
for cmd in curl chafa; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: '$cmd' is required but not installed." >&2
        exit 1
    fi
done

# --- Lightweight JSON Extractor (Pure Bash + grep/sed, no jq needed) ---
json_val() {
    local key="$1"
    grep -oE "\"${key}\"[[:space:]]*:[[:space:]]*\"?[^\",}]+\"?" \
        | head -n 1 \
        | sed -E "s/^\"${key}\"[[:space:]]*:[[:space:]]*\"?//; s/\"?$//"
}

# --- Location Resolution ---
resolve_station() {
    local input="$1"

    case "$input" in
        ALASKA|CARIB|CENTGRLAKES|CONUS|GUAM|HAWAII|NORTHEAST|NORTHROCKIES|PACNORTHWEST|PACSOUTHWEST|SOUTHMissVLY|SOUTHPLAINS|SOUTHEAST|UPPERMISSVLY)
            STATION="$input"
            RESOLVED_LABEL="Mosaic: $input"
            return
            ;;
    esac

    if [[ "$input" =~ ^[0-9]{5}$ ]]; then
        echo "Looking up coordinates for ZIP code $input..."
        local zip_json lat lon points_json
        zip_json=$(curl -sf "https://api.zippopotam.us/us/$input" || true)
        lat=$(printf "%s" "$zip_json" | json_val "latitude")
        lon=$(printf "%s" "$zip_json" | json_val "longitude")

        if [[ -z "$lat" || -z "$lon" ]]; then
            echo "Error: Could not resolve ZIP code '$input'." >&2
            exit 1
        fi

        echo "Finding nearest NWS radar tower for ($lat, $lon)..."
        points_json=$(curl -sf -H "User-Agent: $USER_AGENT" "https://api.weather.gov/points/${lat},${lon}" || true)
        STATION=$(printf "%s" "$points_json" | json_val "radarStation")
        RESOLVED_LABEL="ZIP $input -> $STATION"
        return
    fi

    if [[ "$input" =~ ^[A-Z]{3,4}$ ]]; then
        local icao="$input"
        [[ ${#icao} -eq 3 ]] && icao="K${icao}"

        local station_json
        station_json=$(curl -sf -H "User-Agent: $USER_AGENT" "https://api.weather.gov/stations/${icao}" 2>/dev/null || true)
        if [[ -n "$station_json" ]]; then
            local found
            found=$(printf "%s" "$station_json" | json_val "radarStation")
            if [[ -n "$found" && "$found" != "null" ]]; then
                STATION="$found"
                RESOLVED_LABEL="$icao -> $STATION"
                return
            fi
        fi

        STATION="$icao"
        RESOLVED_LABEL="Station $STATION"
        return
    fi

    STATION="$input"
    RESOLVED_LABEL="$STATION"
}

# --- Pure Bash Binary GIF Frame Splitter ---
# Reads an animated GIF byte stream via od/dd and splits each Graphic Control
# Extension (0x21 0xF9) + Image Descriptor block into standalone valid GIF files
# so chafa can render each frame individually without Python or ImageMagick.
split_gif_pure_bash() {
    local gif_file="$1"
    local out_dir="$2"

    # Read entire file as space-separated hex bytes
    local hex_stream
    hex_stream=$(od -An -tx1 -v "$gif_file" | tr -s ' \n' ' ')
    local -a bytes
    read -ra bytes <<< "$hex_stream"
    local total=${#bytes[@]}

    if (( total < 14 )); then
        return 1
    fi

    # Header (6 bytes) + Logical Screen Descriptor (7 bytes)
    local packed=$(( 16#${bytes[10]} ))
    local gct_flag=$(( (packed >> 7) & 1 ))
    local gct_size=0
    if (( gct_flag == 1 )); then
        local gct_exp=$(( packed & 7 ))
        gct_size=$(( 3 * (1 << (gct_exp + 1)) ))
    fi

    local header_end=$(( 13 + gct_size ))
    if (( header_end >= total )); then
        return 1
    fi

    # Extract shared header + Global Color Table once
    dd if="$gif_file" of="$out_dir/header.bin" bs=1 count="$header_end" 2>/dev/null

    local pos=$header_end
    local idx=0
    local gce_start=-1

    # Helper to skip GIF data sub-blocks starting at $pos
    skip_subblocks() {
        while (( pos < total )); do
            local sz=$(( 16#${bytes[pos]} ))
            (( pos++ ))
            if (( sz == 0 )); then
                break
            fi
            (( pos += sz ))
        done
    }

    while (( pos < total )); do
        local b="${bytes[pos]}"

        if [[ "$b" == "3b" ]]; then
            # GIF Trailer (0x3B)
            break
        elif [[ "$b" == "21" ]]; then
            # Extension Introducer (0x21)
            local label="${bytes[pos+1]:-00}"
            if [[ "$label" == "f9" ]]; then
                # Graphic Control Extension marks start of next frame
                gce_start=$pos
                (( pos += 2 ))
                skip_subblocks
            else
                # Application, Comment, or Plain Text Extension
                (( pos += 2 ))
                skip_subblocks
            fi
        elif [[ "$b" == "2c" ]]; then
            # Image Descriptor (0x2C)
            local frame_start=$pos
            (( gce_start >= 0 )) && frame_start=$gce_start

            local img_packed=$(( 16#${bytes[pos+9]} ))
            local lct_flag=$(( (img_packed >> 7) & 1 ))
            local lct_size=0
            if (( lct_flag == 1 )); then
                local lct_exp=$(( img_packed & 7 ))
                lct_size=$(( 3 * (1 << (lct_exp + 1)) ))
            fi

            # Advance past Image Descriptor (10 bytes) + LCT + LZW Min Code Size (1 byte)
            (( pos += 10 + lct_size + 1 ))
            skip_subblocks

            local frame_len=$(( pos - frame_start ))
            local out_gif
            printf -v out_gif "%s/frame_%03d.gif" "$out_dir" "$idx"

            cp "$out_dir/header.bin" "$out_gif"
            dd if="$gif_file" bs=1 skip="$frame_start" count="$frame_len" >> "$out_gif" 2>/dev/null
            printf '\x3b' >> "$out_gif"

            (( idx++ ))
            gce_start=-1
        else
            (( pos++ ))
        fi
    done

    echo "$idx"
}

# --- Fetch & Pre-Render Frames ---
fetch_and_render() {
    local url="https://radar.weather.gov/ridge/standard/${STATION}_loop.gif"
    local raw_gif="$TMP_DIR/loop.gif"
    local hdr_file="$TMP_DIR/headers.txt"

    rm -f "$TMP_DIR"/frame_*.gif "$raw_gif" "$hdr_file"

    if ! curl -sfL -D "$hdr_file" -H "User-Agent: $USER_AGENT" "$url" -o "$raw_gif"; then
        return 1
    fi

    # Parse HTTP Last-Modified header into epoch seconds for frame timestamps
    local last_mod_str
    last_mod_str=$(grep -i '^last-modified:' "$hdr_file" | head -n 1 | sed -E 's/^[Ll]ast-[Mm]odified:[[:space:]]*//; s/\r$//')
    if [[ -n "$last_mod_str" ]]; then
        LAST_MOD_EPOCH=$(date -d "$last_mod_str" +%s 2>/dev/null || date +%s)
    else
        LAST_MOD_EPOCH=$(date +%s)
    fi

    # Determine terminal dimensions
    local cols rows target_size
    if [[ -n "$SIZE" ]]; then
        target_size="$SIZE"
    else
        cols=$(tput cols 2>/dev/null || echo 80)
        rows=$(tput lines 2>/dev/null || echo 36)
        local max_h=$(( rows - 4 ))
        (( max_h < 12 )) && max_h=12
        local max_w=$(( max_h * 2 ))
        (( max_w > cols )) && max_w=$cols
        target_size="${max_w}x${max_h}"
    fi

    # Split GIF into individual frames in pure Bash
    local count
    count=$(split_gif_pure_bash "$raw_gif" "$TMP_DIR" || echo "0")

    FRAMES=()
    TIMESTAMPS=()

    if (( count > 0 )); then
        FRAME_COUNT=$count
        local i
        # NWS standard loops update roughly every 5 minutes (300s) leading up to Last-Modified
        for (( i=0; i<FRAME_COUNT; i++ )); do
            local f_path
            printf -v f_path "%s/frame_%03d.gif" "$TMP_DIR" "$i"
            FRAMES+=("$(chafa --symbols "$SYMBOL_MODE" --colors "$COLOR_MODE" --size "$target_size" "$f_path")")

            local offset_sec=$(( (FRAME_COUNT - 1 - i) * 300 ))
            local frame_epoch=$(( LAST_MOD_EPOCH - offset_sec ))
            local local_ts utc_ts
            local_ts=$(date -d "@$frame_epoch" "+%Y-%m-%d %H:%M %Z" 2>/dev/null || date "+%Y-%m-%d %H:%M %Z")
            utc_ts=$(date -u -d "@$frame_epoch" "+%H:%MZ" 2>/dev/null || date -u "+%H:%MZ")
            TIMESTAMPS+=("${local_ts} (${utc_ts}) [est.]")
        done
    else
        # Fallback to single static image if station has no active multi-frame loop
        local static_url="https://radar.weather.gov/ridge/standard/${STATION}_0.gif"
        curl -sfL -H "User-Agent: $USER_AGENT" "$static_url" -o "$raw_gif" || return 1
        FRAME_COUNT=1
        FRAMES+=("$(chafa --symbols "$SYMBOL_MODE" --colors "$COLOR_MODE" --size "$target_size" "$raw_gif")")
        local local_ts utc_ts
        local_ts=$(date -d "@$LAST_MOD_EPOCH" "+%Y-%m-%d %H:%M %Z" 2>/dev/null || date "+%Y-%m-%d %H:%M %Z")
        utc_ts=$(date -u -d "@$LAST_MOD_EPOCH" "+%H:%MZ" 2>/dev/null || date -u "+%H:%MZ")
        TIMESTAMPS+=("${local_ts} (${utc_ts})")
    fi

    return 0
}

# --- Main Execution ---
TMP_DIR=$(mktemp -d)
resolve_station "$TARGET"

echo "Fetching radar loop for $RESOLVED_LABEL..."
if ! fetch_and_render; then
    echo "Error: Failed to retrieve radar imagery for '$STATION'." >&2
    exit 1
fi

# Hide cursor and enable non-canonical single-key reading
printf "\033[?25l"
stty -echo -icanon time 0 min 0 2>/dev/null || true

idx=0
paused=0
status_note=""

clear

while true; do
    # Build current frame output in a single buffer for tear-free terminal updates
    local_state="PLAYING"
    (( paused == 1 )) && local_state="PAUSED "

    fps_x10=$(( 10000 / DELAY_MS ))
    fps_str="$(( fps_x10 / 10 )).$(( fps_x10 % 10 ))"

    header_line=$(printf "\033[1;36mNWS Radar:\033[0m %-22s \033[1;33mFrame:\033[0m %02d/%02d  \033[1;32mTime:\033[0m %s" \
        "$RESOLVED_LABEL" "$((idx + 1))" "$FRAME_COUNT" "${TIMESTAMPS[$idx]}")
    footer_line=$(printf "\033[7m %s \033[0m Speed: %dms (%s FPS) | [+] Faster  [-] Slower  [r] Refresh  [Space] Pause  [q] Quit %s" \
        "$local_state" "$DELAY_MS" "$fps_str" "$status_note")

    # Move cursor to home (1,1), print header + pre-rendered frame + footer, clear trailing lines
    printf "\033[H%s\033[K\n%s\n%s\033[K\033[J" "$header_line" "${FRAMES[$idx]}" "$footer_line"
    status_note=""

    # Format delay in seconds for bash `read -t`
    printf -v sleep_sec "%d.%03d" "$(( DELAY_MS / 1000 ))" "$(( DELAY_MS % 1000 ))"

    key=""
    if IFS= read -rsn1 -t "$sleep_sec" key; then
        case "$key" in
            "+"|"=")
                (( DELAY_MS -= STEP_DELAY_MS ))
                (( DELAY_MS < MIN_DELAY_MS )) && DELAY_MS=$MIN_DELAY_MS
                ;;
            "-"|"_")
                (( DELAY_MS += STEP_DELAY_MS ))
                (( DELAY_MS > MAX_DELAY_MS )) && DELAY_MS=$MAX_DELAY_MS
                ;;
            "r"|"R")
                printf "\033[H\033[1;33mRefreshing radar loop for %s...\033[K\033[0m" "$RESOLVED_LABEL"
                if fetch_and_render; then
                    idx=0
                    status_note="[Refreshed $(date +%H:%M:%S)]"
                else
                    status_note="[Refresh failed]"
                fi
                clear
                ;;
            " ")
                paused=$(( 1 - paused ))
                ;;
            "q"|"Q")
                break
                ;;
        esac
    fi

    if (( paused == 0 && FRAME_COUNT > 0 )); then
        idx=$(( (idx + 1) % FRAME_COUNT ))
    fi
done
