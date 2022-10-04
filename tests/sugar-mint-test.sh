#!/bin/bash

ENV_URL="devnet"
RPC="https://devnet.genesysgo.net"
STORAGE="bundlr"

SCRIPT_DIR=$(cd -- $(dirname -- "${BASH_SOURCE[0]}") &>/dev/null && pwd)
PARENT_DIR="$(dirname "$SCRIPT_DIR")"
SUGAR_BIN="cargo run --release --bin sugar --"
CM_CREATOR="keypairs/cm-creator-keypair.json"
ASSETS_DIR="assets"
CONFIG_FILE="config.json"
CONFIG_FILE_BACKUP="config.json.backup"
AIRDROP_LIST="airdrop-list.json"
AIRDROP_RESULTS="airdrop_results.json"
CACHE_FILE="cache.json"

\rm -fr ${AIRDROP_RESULTS} ${CACHE_FILE}
\cp -f ${CONFIG_FILE_BACKUP} ${CONFIG_FILE}

$SUGAR_BIN launch -c ${CONFIG_FILE} --keypair ${CM_CREATOR} -r ${RPC} ${ASSETS_DIR}
$SUGAR_BIN mint --keypair ${CM_CREATOR} -r ${RPC} --airdrop-list ${AIRDROP_LIST}

TARGET_1=$(jq -r '."9kEvUrVDiJAD2wrPHF3Jv9cFfXZpid9Gca51vkV4g7H2" | length' ${AIRDROP_RESULTS})
TARGET_2=$(jq -r '."6Jex93Vgk7zhoiBatP1PHw9Uc29FbjQQP7BkPV7Kmb9R" | length' ${AIRDROP_RESULTS})

if [ "$TARGET_1" -ne 5 ] ; then
    echo "Airdrop results are not correct for target-1"
    exit 1
fi

if [ "$TARGET_2" -ne 1 ]; then
    echo "Airdrop results are not correct for target-2"
    exit 1
fi

echo "[$(date "+%T")] Test completed"