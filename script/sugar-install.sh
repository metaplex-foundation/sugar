#!/bin/bash
#
# Sugar CLI binary installation script
# ------------------------------------
#
# The purpose of this script is to automate the download and installation
# of Sugar CLI binary.
#
# The script does a (simple) platform detection, downloads the binary
# for the detected platform and copies it to a folder in the PATH variable.
# 
# Currently the supported platforms are macOS, Linux, or another Unix-like OS
# running variations of the sh Unix shells.
#

RED() { echo $'\e[1;31m'$1$'\e[0m'; }
GRN() { echo $'\e[1;32m'$1$'\e[0m'; }
CYN() { echo $'\e[1;36m'$1$'\e[0m'; }

abort_on_error() {
    if [ ! $1 -eq 0 ]; then
        RED "Aborting: operation failed"
        exit 1
    fi
}

CYN  "🍬 Sugar CLI binary installation script"
echo "---------------------------------------"
echo ""

OS_FLAVOUR="$(uname -s)"
PROCESSOR="$(uname -m)"

# we need to check whether we are running on an ARM
# architecture or not

case "$PROCESSOR" in
    arm* | aarch* | ppc* )
        echo "Binary for $PROCESSOR architecture is not currently supported. Plese follow the instructions at:"
        echo "  => $(CYN https://github.com/metaplex-foundation/sugar)"
        echo ""
        echo "to build Sugar from the source code."
        exit 1
        ;;

    *)
        # good to go
        ;;
esac
 
BIN="sugar"
VERSION="ubuntu-latest"

if [ "$OS_FLAVOUR" = Darwin ]; then
    VERSION="macos-latest"
fi

DIST="$VERSION.zip"
# creates a temporary directory to save the distribution file
SOURCE="$(mktemp -d)"

CYN "1. Downloading distribution"
echo ""

# downloads the distribution file
REMOTE="https://github.com/metaplex-foundation/sugar/releases/latest/download/"
curl -L $REMOTE$DIST --output "$SOURCE/$DIST"

SIZE=$(wc -c "$SOURCE/$DIST" | grep -oE "[0-9]+" | head -n 1)

if [ $SIZE -eq 0 ]; then
    RED "Aborting: could not download Sugar distribution"
    exit 1
fi

if [ "$(command -v unzip)" = "" ]; then
    RED "Aborting: required 'unzip' command could not be found"
    exit 1
fi

echo ""
CYN "2. Extracting"
echo ""

unzip "$SOURCE/$DIST" -d "$SOURCE"
abort_on_error $?

# makes sure the binary will be executable
chmod u+x "$SOURCE/$BIN-$VERSION"
abort_on_error $?

echo ""
CYN "3. Moving binary into place"
echo ""

if [ ! "$(command -v $BIN)" = "" ]; then
    # binary already found on system, ask if we should
    # replace it
    EXISTING="$(which $BIN)"

    echo "Sugar binary was found at:"
    echo "  => $(CYN $EXISTING)"
    echo ""
    echo -n "$(CYN "Replace it? [Y/n]") (default 'n'): "
    read REPLACE

    if [ -z "REPLACE" ]; then
        REPLACE="n"
    fi

    if [ "$REPLACE" = Y ]; then
        echo ""
        echo -n "'$BIN' will be moved to '$(dirname "$EXISTING")'"

        case "$EXISTING" in
            *local* )
                echo ". You may be asked to enter your password to confirm the replacement."
                CMD="sudo mv"
                ;;
            *)
                CMD="mv"
                ;;
        esac

        echo ""
        $CMD "$SOURCE/$BIN-$VERSION" "$EXISTING"
        abort_on_error $?
    else
        # nothing else to do, replacement was cancelled
        RED "Aborting: replacement cancelled"
        exit 1
    fi
else
    # determines a suitable directory for the binary - preference:
    # 1) ~/.cargo/bin if exists
    # 2) /usr/local/bin otherwise
    TARGET="$HOME/.cargo/bin"

    if [ ! -d "$TARGET" ]; then
        TARGET="/usr/local/bin"
        CMD="sudo mv"
    fi

    echo -n "'$BIN' will be moved to '$TARGET'"

    if [ -z ${CMD+x} ]; then
        CMD="mv"
    else
        echo ". You may be asked to enter your password to confirm the replacement."
    fi

    echo ""
    $CMD "$SOURCE/$BIN-$VERSION" "$TARGET/$BIN"
    abort_on_error $?
fi

# sanity check
if [ "$(command -v $BIN)" = "" ]; then
    # installation was not completed
    echo ""
    RED "Could not install Sugar in '$TARGET'"
    exit 1
else
    # success
    echo "$(GRN "Installation completed:") type '$BIN' to start using it."
fi