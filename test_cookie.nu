# ---- CONFIG ----
# PUT YOUR SAPISID HERE (AUTHENTICATION header - browse?)
let sapisid = "nJFLTMlo2TZW/ASa3OwvPGM"
let origin = "https://music.youtube.com"

# PUT YOUR COOKIE (browse?) HERE
let cookie = ""

# ---- GENERATE SAPISIDHASH ----
let timestamp = (date now | format date "%s")

let input = $"($timestamp) ($sapisid) ($origin)"

let hash = (
    echo $input 
    | openssl sha1 
    | str replace "SHA1(stdin)= " ""
)

let authorization = $"SAPISIDHASH ($timestamp)_($hash)"

# ---- REQUEST ----
http post --content-type application/json "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false" --headers [cookie $cookie Authorization $authorization Content-Type "application/json" Origin $origin Referer "https://music.youtube.com/" User-Agent "Mozilla/5.0"] {context: {client: {clientName: "WEB_REMIX", clientVersion: "1.20240101.01.00"}}}
