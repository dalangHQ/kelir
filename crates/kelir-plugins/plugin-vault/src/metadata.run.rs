// see: cans/plugins/vault.md#Metadata-service-(vault)
// TODO (L3) [governance_vault]: implement metadata HTTP response writer / JSON encoder.; slop: metadata responses are never written, clients receive no JSON, the metadata endpoint silently serves nothing.
// TODO (L3) [governance_vault]: metadata HTTP response writer / JSON encoder replacing the hand-rolled ok_response / error_response / http_response / json_string framing; slop: hand-rolled escaping ships, crafted value breaks the JSON string
