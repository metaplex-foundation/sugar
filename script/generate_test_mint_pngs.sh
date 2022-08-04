
total_collection_count=${3:-10}
for i in $( eval echo {01..$total_collection_count} )
do
    file_index=$(($i - 1))
    printf -v padded_number "%05d" $i
    cp $1 "${2}/${file_index}.png"
    jq --null-input \
    --arg i "$i" \
    --arg padded_number "Number #$padded_number" \
    --arg file_index "$file_index" \
    --arg description "Collection of ${total_collection_count} test numbers on the blockchain. This is the number ${i}/${total_collection_count}." \
    --arg image "${file_index}.png" \
    '{
        "name": $padded_number,
        "symbol": "NB",
        "description": $description,
        "image": $image,
        "attributes": [
        {
            "trait_type": "Number",
            "value": $file_index
        }
        ],
        "properties": {
        "files": [
            {
            "uri": $image,
            "type": "image/png"
            }
        ]
        }
    }' > "${2}/${file_index}.json"
done