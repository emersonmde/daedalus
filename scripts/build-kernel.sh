#!/usr/bin/env bash
# Build and deploy DaedalusOS kernel for Raspberry Pi 4
#
# Usage:
#   ./scripts/build-kernel.sh                      # Build only
#   ./scripts/build-kernel.sh --deploy <VOLUME>    # Build and deploy to SD card
#   ./scripts/build-kernel.sh --setup <VOLUME>     # Setup SD card with firmware + kernel
#   ./scripts/build-kernel.sh --list-volumes       # List available volumes

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Paths
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL_ELF="${PROJECT_ROOT}/target/aarch64-daedalus/release/daedalus"
KERNEL_BIN="${PROJECT_ROOT}/target/aarch64-daedalus/release/kernel8.img"

# SD card mount point (set by command line argument)
SD_MOUNT=""

# Firmware URLs
FIRMWARE_BASE="https://github.com/raspberrypi/firmware/raw/master/boot"
FIRMWARE_FILES=(
    "start4.elf"
    "fixup4.dat"
    "bcm2711-rpi-4-b.dtb"
)

# ============================================================================
# Helper Functions
# ============================================================================

print_step() {
    echo -e "${BLUE}==>${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1" >&2
}

print_bold() {
    echo -e "${BOLD}$1${NC}"
}

# ============================================================================
# Safety Functions
# ============================================================================

list_external_volumes() {
    echo ""
    print_bold "Available External Volumes:"
    echo ""

    # List all volumes, excluding system volumes
    local found_volumes=0

    for vol in /Volumes/*; do
        # Skip if not a directory
        [ -d "$vol" ] || continue

        # Get volume name
        local vol_name=$(basename "$vol")

        # Skip macOS system volumes
        case "$vol_name" in
            "Macintosh HD"|"Macintosh HD - Data"|"Preboot"|"Recovery"|"VM"|"Update"|"Data")
                continue
                ;;
        esac

        # Get disk info
        local disk_info=$(diskutil info "$vol" 2>/dev/null || echo "")

        # Check if it's external/removable
        # Matches: "Protocol: External", "Removable Media: Yes", "Removable Media: Removable"
        if echo "$disk_info" | grep -qE "Protocol.*(External|Secure Digital)|Removable Media.*(Yes|Removable)"; then
            found_volumes=1

            # Extract useful info
            local fs_type=$(echo "$disk_info" | grep "Type (Bundle):" | awk -F: '{print $2}' | xargs)
            local size=$(echo "$disk_info" | grep "Disk Size:" | awk -F: '{print $2}' | xargs)
            local device=$(echo "$disk_info" | grep "Device Node:" | awk -F: '{print $2}' | xargs)

            echo -e "${GREEN}✓${NC} ${BOLD}$vol_name${NC}"
            echo "    Path:       $vol"
            echo "    Device:     $device"
            echo "    Filesystem: $fs_type"
            echo "    Size:       $size"
            echo ""
        fi
    done

    if [ $found_volumes -eq 0 ]; then
        print_warning "No external volumes found"
        echo ""
    fi
}

verify_volume_safe() {
    local vol_path="$1"

    # Check volume exists
    if [ ! -d "$vol_path" ]; then
        print_error "Volume does not exist: $vol_path"
        return 1
    fi

    # Get volume name
    local vol_name=$(basename "$vol_path")

    # CRITICAL: Block any system volumes
    case "$vol_name" in
        ""|"/"|"Macintosh HD"|"Macintosh HD - Data"|"Preboot"|"Recovery"|"System"|"VM"|"Update"|"Data"|"Library"|"Applications"|"Users")
            print_error "BLOCKED: '$vol_name' is a system volume!"
            print_error "This script will not write to system volumes for safety."
            return 1
            ;;
    esac

    # Check if volume path is under /Volumes (macOS external volumes)
    if [[ "$vol_path" != /Volumes/* ]]; then
        print_error "BLOCKED: Volume must be under /Volumes/"
        print_error "Got: $vol_path"
        return 1
    fi

    # Get disk info
    local disk_info=$(diskutil info "$vol_path" 2>/dev/null || echo "")

    if [ -z "$disk_info" ]; then
        print_error "Cannot get disk info for: $vol_path"
        return 1
    fi

    # Verify it's external or removable
    # Matches: "Protocol: External", "Protocol: Secure Digital", "Removable Media: Yes/Removable"
    if ! echo "$disk_info" | grep -qE "Protocol.*(External|Secure Digital)|Removable Media.*(Yes|Removable)"; then
        print_error "BLOCKED: Volume does not appear to be external/removable"
        print_warning "For safety, this script only writes to external volumes."
        echo ""
        echo "Disk info:"
        echo "$disk_info" | grep -E "Protocol|Removable"
        return 1
    fi

    # Check filesystem type (should be FAT32 for Pi boot)
    local fs_type=$(echo "$disk_info" | grep "Type (Bundle):" | awk -F: '{print $2}' | xargs)
    # macOS reports FAT32 as "msdos", "MS-DOS", "FAT", or "FAT32"
    if [[ "$fs_type" != *"FAT"* && "$fs_type" != *"MS-DOS"* && "$fs_type" != "msdos" ]]; then
        print_warning "Volume filesystem is '$fs_type', not FAT32"
        print_warning "Raspberry Pi boot partitions should be FAT32"
        echo ""
        read -p "Continue anyway? (yes/no): " confirm
        if [ "$confirm" != "yes" ]; then
            print_error "Aborted by user"
            return 1
        fi
    fi

    # Check volume size (boot partitions are usually small, < 1 GB)
    local size_bytes=$(echo "$disk_info" | grep "Disk Size:" | grep -o '[0-9]*' | head -1)
    if [ -n "$size_bytes" ] && [ "$size_bytes" -gt 10000000000 ]; then
        local size_gb=$((size_bytes / 1000000000))
        print_warning "Volume is ${size_gb} GB - larger than typical boot partition"
        echo ""
        read -p "Are you sure this is the correct SD card? (yes/no): " confirm
        if [ "$confirm" != "yes" ]; then
            print_error "Aborted by user"
            return 1
        fi
    fi

    return 0
}

confirm_volume() {
    local vol_path="$1"
    local vol_name=$(basename "$vol_path")

    echo ""
    print_warning "You are about to write to: ${BOLD}$vol_name${NC}"
    print_warning "Path: $vol_path"
    echo ""
    print_bold "This will modify/overwrite files on this volume!"
    echo ""
    read -p "Type the volume name to confirm: " confirm

    if [ "$confirm" != "$vol_name" ]; then
        print_error "Volume name mismatch. Aborting."
        exit 1
    fi

    print_success "Confirmed: $vol_name"
}

set_sd_mount() {
    local volume_name="$1"

    # Construct full path
    SD_MOUNT="/Volumes/$volume_name"

    # Verify safety (checks if removable/external)
    if ! verify_volume_safe "$SD_MOUNT"; then
        exit 1
    fi
}

# ============================================================================
# Build Functions
# ============================================================================

build_kernel() {
    print_step "Building kernel (release mode)..."
    cargo build --release
    print_success "Kernel built: ${KERNEL_ELF}"
}

convert_to_binary() {
    print_step "Converting ELF to raw binary..."

    if ! command -v rust-objcopy &> /dev/null; then
        print_error "rust-objcopy not found. Is Rust installed?"
        exit 1
    fi

    rust-objcopy -O binary "${KERNEL_ELF}" "${KERNEL_BIN}"

    local size=$(ls -lh "${KERNEL_BIN}" | awk '{print $5}')
    print_success "Binary created: ${KERNEL_BIN} (${size})"
}

# ============================================================================
# SD Card Functions
# ============================================================================

check_sd_card() {
    if [ -z "$SD_MOUNT" ]; then
        print_error "SD card volume not set"
        exit 1
    fi

    if [ ! -d "${SD_MOUNT}" ]; then
        print_error "SD card not mounted at ${SD_MOUNT}"
        print_warning "Please insert SD card and try again"
        exit 1
    fi
    print_success "SD card found at ${SD_MOUNT}"
}

deploy_kernel() {
    check_sd_card

    print_step "Deploying kernel to SD card..."
    cp "${KERNEL_BIN}" "${SD_MOUNT}/kernel8.img"

    local size=$(ls -lh "${SD_MOUNT}/kernel8.img" | awk '{print $5}')
    print_success "Kernel deployed: ${SD_MOUNT}/kernel8.img (${size})"
}

download_firmware() {
    check_sd_card

    print_step "Downloading firmware files..."

    cd "${SD_MOUNT}"
    for file in "${FIRMWARE_FILES[@]}"; do
        if [ -f "${file}" ]; then
            print_warning "${file} already exists, skipping"
        else
            echo "  Downloading ${file}..."
            curl -L -O "${FIRMWARE_BASE}/${file}"
            print_success "Downloaded ${file}"
        fi
    done
    cd "${PROJECT_ROOT}"
}

create_config_txt() {
    check_sd_card

    print_step "Creating config.txt..."

    cat > "${SD_MOUNT}/config.txt" << 'EOF'
# DaedalusOS Bare-Metal Boot Configuration
# For Raspberry Pi 4 Model B

# Boot in 64-bit mode
arm_64bit=1

# Load our kernel
kernel=kernel8.img

# Enable UART for serial console (PL011)
enable_uart=1

# Disable Bluetooth to free up PL011 UART
# (Maps PL011 to GPIO 14/15 on header)
dtoverlay=disable-bt

# Disable rainbow splash screen
disable_splash=1

# Set GPU memory to minimum (bare-metal doesn't use GPU)
gpu_mem=16
EOF

    print_success "config.txt created"
}

setup_sd_card() {
    check_sd_card

    echo ""
    echo "========================================="
    echo "  SD Card Setup for DaedalusOS"
    echo "========================================="
    echo ""

    download_firmware
    create_config_txt
    deploy_kernel

    echo ""
    print_success "SD card setup complete!"
    echo ""
    show_sd_card_contents
}

show_sd_card_contents() {
    check_sd_card

    echo "SD card contents:"
    echo ""
    ls -lh "${SD_MOUNT}" | grep -E '(kernel8.img|config.txt|start4.elf|fixup4.dat|bcm2711)' || true
    echo ""
}

eject_sd_card() {
    check_sd_card

    print_step "Ejecting SD card..."
    diskutil eject "${SD_MOUNT}"
    print_success "SD card ejected safely"
}

# ============================================================================
# Main
# ============================================================================

show_usage() {
    cat << EOF
${BOLD}DaedalusOS Kernel Build & Deploy${NC}

${BOLD}Usage:${NC}
  $0 [OPTION] [VOLUME_NAME]

${BOLD}Options:${NC}
  (none)                   Build kernel binary only
  --deploy VOLUME_NAME     Build and deploy to SD card
  --setup VOLUME_NAME      Setup SD card (firmware + config + kernel)
  --list-volumes, -l       List available external volumes
  --eject VOLUME_NAME      Safely eject SD card
  --help, -h               Show this help message

${BOLD}Examples:${NC}
  $0                               # Just build the kernel
  $0 --list-volumes                # List available volumes
  $0 --deploy "NO NAME"            # Build and copy to SD card
  $0 --setup "BOOT"                # First-time SD card setup
  $0 --eject "NO NAME"             # Eject SD card safely

${BOLD}Safety Features:${NC}
  - Requires explicit volume name (no hardcoded defaults)
  - Blocks system volumes (Macintosh HD, etc.)
  - Verifies volume is external/removable
  - Checks filesystem type (warns if not FAT32)
  - Requires confirmation before writing

${BOLD}SD Card Setup:${NC}
  The --setup option downloads firmware files from the official
  Raspberry Pi repository and creates a bootable SD card.

  Required files:
    - start4.elf          (GPU firmware)
    - fixup4.dat          (GPU memory config)
    - bcm2711-rpi-4-b.dtb (Device tree blob)
    - config.txt          (Boot configuration)
    - kernel8.img         (Your DaedalusOS kernel)

EOF
}

main() {
    cd "${PROJECT_ROOT}"

    local mode="${1:-build}"
    local volume_name="${2:-}"

    case "${mode}" in
        --list-volumes|-l)
            list_external_volumes
            exit 0
            ;;

        --deploy|-d)
            if [ -z "$volume_name" ]; then
                print_error "Volume name required for --deploy"
                echo ""
                echo "Usage: $0 --deploy VOLUME_NAME"
                echo ""
                echo "Available volumes:"
                list_external_volumes
                exit 1
            fi

            echo ""
            echo "DaedalusOS Kernel Build & Deploy"
            echo "================================="
            echo ""

            set_sd_mount "$volume_name"
            build_kernel
            convert_to_binary
            deploy_kernel

            echo ""
            print_success "Build and deploy complete!"
            echo ""
            print_warning "Remember to eject SD card before removing:"
            echo "  $0 --eject \"$volume_name\""
            echo ""
            ;;

        --setup|-s)
            if [ -z "$volume_name" ]; then
                print_error "Volume name required for --setup"
                echo ""
                echo "Usage: $0 --setup VOLUME_NAME"
                echo ""
                echo "Available volumes:"
                list_external_volumes
                exit 1
            fi

            echo ""
            echo "DaedalusOS Kernel Build & Deploy"
            echo "================================="
            echo ""

            set_sd_mount "$volume_name"
            build_kernel
            convert_to_binary
            setup_sd_card

            echo ""
            print_warning "Remember to eject SD card before removing:"
            echo "  $0 --eject \"$volume_name\""
            echo ""
            ;;

        --eject|-e)
            if [ -z "$volume_name" ]; then
                print_error "Volume name required for --eject"
                echo ""
                echo "Usage: $0 --eject VOLUME_NAME"
                exit 1
            fi

            SD_MOUNT="/Volumes/$volume_name"
            eject_sd_card
            ;;

        --help|-h)
            show_usage
            exit 0
            ;;

        *)
            echo ""
            echo "DaedalusOS Kernel Build & Deploy"
            echo "================================="
            echo ""

            build_kernel
            convert_to_binary

            echo ""
            print_success "Build complete!"
            echo ""
            echo "Next steps:"
            echo "  1. List volumes:  $0 --list-volumes"
            echo "  2. Deploy:        $0 --deploy \"VOLUME_NAME\""
            echo "  3. Or setup:      $0 --setup \"VOLUME_NAME\""
            echo ""
            ;;
    esac
}

main "$@"
