#!/usr/bin/env bash
# Interop checks for std::tls and std::http against other implementations
# (WI-7, CHG-0266), on loopback only:
#   - std::tls's client against `openssl s_server`: every cipher suite and
#     key-exchange group std offers, a HelloRetryRequest, RSA, P-256, P-384
#     and Ed25519 certificates, chains of depth 1 to 3, a wrong host name
#     and an expired certificate (both refused);
#   - `openssl s_client` against std::tls's server: each suite and group,
#     each key type, a chain with an intermediate;
#   - curl against std::http's server: GET, HEAD, two requests on one kept-
#     alive connection, a large body, a chunked upload, large headers;
#   - std::http's client against `python3 -m http.server` and a chunked
#     answer.
# Certificates are made fresh in a temporary directory with openssl.
# Needs openssl, curl, python3 and the release cobc. Prints one line per
# check; exit status 1 when any failed, 2 when a tool is missing.
set -u
H="$(cd "$(dirname "$0")" && pwd)"
R="$(cd "$H/../../.." && pwd)"
COBC="$R/impl/target/release/cobc"
for t in openssl curl python3; do command -v $t >/dev/null || { echo "interop: $t not found"; exit 2; }; done
W="$(mktemp -d "${TMPDIR:-/tmp}/cobaltc-interop.XXXXXX")"
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$W"' EXIT
cd "$W"
ulimit -t 3600
PASS=0; FAIL=0
ok()   { PASS=$((PASS + 1)); echo "  ok    $*"; }
bad()  { FAIL=$((FAIL + 1)); echo "  FAIL  $*"; }

echo "== build"
for p in tls_client tls_server http_server http_client; do
    "$COBC" --cc gcc -o "$W/$p" "$H/$p.cb" > "$W/build_$p.log" 2>&1 || { echo "interop: building $p failed"; cat "$W/build_$p.log"; exit 1; }
done

echo "== certificates"
key() { # NAME KIND
    case "$2" in
        rsa)     openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$1.key" 2>/dev/null ;;
        p256)    openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-256 -out "$1.key" 2>/dev/null ;;
        p384)    openssl genpkey -algorithm EC -pkeyopt ec_paramgen_curve:P-384 -out "$1.key" 2>/dev/null ;;
        ed25519) openssl genpkey -algorithm ED25519 -out "$1.key" 2>/dev/null ;;
    esac
}
printf 'basicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n' > ca.ext
printf 'basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost\n' > leaf.ext
key ca p256
openssl req -x509 -new -key ca.key -subj "/CN=interop root" -days 3650 -addext basicConstraints=critical,CA:TRUE -addext keyUsage=critical,keyCertSign,cRLSign -out ca.pem 2>/dev/null
sign() { # NAME ISSUER EXT
    openssl req -new -key "$1.key" -subj "/CN=$1" -out "$1.csr" 2>/dev/null
    openssl x509 -req -in "$1.csr" -CA "$2.pem" -CAkey "$2.key" -CAcreateserial -days 365 -extfile "$3" -out "$1.pem" 2>/dev/null
}
for k in rsa p256 p384 ed25519; do key "leaf_$k" $k; sign "leaf_$k" ca leaf.ext; done
key int1 p256; sign int1 ca ca.ext
key int2 p256; sign int2 int1 ca.ext
key leaf_d2 p256; sign leaf_d2 int1 leaf.ext
key leaf_d3 p256; sign leaf_d3 int2 leaf.ext
cat int1.pem > chain_d2.pem
cat int2.pem int1.pem > chain_d3.pem
# An expired leaf: `openssl ca` takes explicit dates.
mkdir -p ca_db; : > ca_db/index.txt; echo 1000 > ca_db/serial
cat > ca.cnf <<EOF
[ ca ]
default_ca = d
[ d ]
database = ca_db/index.txt
serial = ca_db/serial
new_certs_dir = ca_db
default_md = sha256
policy = p
copy_extensions = none
x509_extensions = leaf
[ p ]
commonName = supplied
[ leaf ]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost
EOF
key leaf_old p256
openssl req -new -key leaf_old.key -subj "/CN=old" -out leaf_old.csr 2>/dev/null
openssl ca -batch -config ca.cnf -cert ca.pem -keyfile ca.key -startdate 20200101000000Z -enddate 20210101000000Z -in leaf_old.csr -out leaf_old.pem >/dev/null 2>&1
for f in leaf_rsa leaf_p256 leaf_p384 leaf_ed25519 leaf_d2 leaf_d3 leaf_old; do [ -s "$f.pem" ] || { echo "interop: making $f.pem failed"; exit 1; }; done

port() { python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()'; }

# std::tls client against s_server -www, which answers an HTTP request.
s_server() { # PORT CERT KEY [CHAIN] [s_server options...]
    local p=$1 c=$2 k=$3; shift 3
    openssl s_server -accept "$p" -cert "$c" -key "$k" -www -naccept 1 "$@" > s_server_$p.log 2>&1 &
    for _ in $(seq 50); do (exec 3<>/dev/tcp/127.0.0.1/$p) 2>/dev/null && return 0; sleep 0.1; done
}
client_case() { # LABEL WANT NAME CERT KEY [s_server options...]
    local label=$1 want=$2 name=$3 c=$4 k=$5; shift 5
    local p; p=$(port)
    openssl s_server -accept "$p" -cert "$c" -key "$k" -www -naccept 1 "$@" > s_server.log 2>&1 &
    local pid=$!
    for _ in $(seq 50); do sleep 0.1; grep -q ACCEPT s_server.log && break; done
    local got; got=$(timeout -k 5 60 "$W/tls_client" "$p" "$W/ca.pem" "$name" 2>&1 | head -n 1)
    kill $pid 2>/dev/null; wait $pid 2>/dev/null
    case "$got" in $want*) ok "client: $label ($got)" ;; *) bad "client: $label: want $want, got: $got" ;; esac
}
echo "== std::tls client against openssl s_server"
for cs in TLS_AES_128_GCM_SHA256 TLS_AES_256_GCM_SHA384 TLS_CHACHA20_POLY1305_SHA256; do
    client_case "suite $cs" "OK HTTP/1.0 200" localhost leaf_p256.pem leaf_p256.key -ciphersuites "$cs"
done
for g in X25519 P-256 P-384; do
    client_case "group $g" "OK HTTP/1.0 200" localhost leaf_p256.pem leaf_p256.key -groups "$g"
done
client_case "HelloRetryRequest (server takes P-384 only)" "OK HTTP/1.0 200" localhost leaf_p256.pem leaf_p256.key -groups P-384 -no_middlebox
for k in rsa p384 ed25519; do
    client_case "$k certificate" "OK HTTP/1.0 200" localhost leaf_$k.pem leaf_$k.key
done
client_case "chain depth 2" "OK HTTP/1.0 200" localhost leaf_d2.pem leaf_d2.key -cert_chain chain_d2.pem
client_case "chain depth 3" "OK HTTP/1.0 200" localhost leaf_d3.pem leaf_d3.key -cert_chain chain_d3.pem
client_case "wrong host name refused" "ERR certificate is not for this host" other.example leaf_p256.pem leaf_p256.key
client_case "expired certificate refused" "ERR certificate expired" localhost leaf_old.pem leaf_old.key

# openssl s_client against std::tls's server.
echo "== openssl s_client against std::tls server"
server_case() { # LABEL CERTFILE KEYFILE [s_client options...]
    local label=$1 c=$2 k=$3; shift 3
    : > tls_server.out
    "$W/tls_server" "$k" "$c" 1 > tls_server.out 2>&1 &
    local pid=$!
    for _ in $(seq 100); do sleep 0.1; grep -q '^port' tls_server.out && break; done
    local p; p=$(sed -n 's/^port //p' tls_server.out)
    local got; got=$( (echo "hello $label"; sleep 2) | timeout -k 5 60 openssl s_client -connect "127.0.0.1:$p" -servername localhost -CAfile "$W/ca.pem" -verify_return_error -quiet "$@" 2>s_client.err | head -n 1)
    wait $pid 2>/dev/null
    if [ "$got" = "echo hello $label" ]; then ok "server: $label"; else bad "server: $label: got '$got'; $(tail -n 2 s_client.err | tr '\n' ' ') $(tr '\n' ' ' < tls_server.out)"; fi
}
for cs in TLS_AES_128_GCM_SHA256 TLS_AES_256_GCM_SHA384 TLS_CHACHA20_POLY1305_SHA256; do
    server_case "suite $cs" leaf_p256.pem leaf_p256.key -ciphersuites "$cs"
done
for g in X25519 P-256 P-384; do
    server_case "group $g" leaf_p256.pem leaf_p256.key -groups "$g"
done
for k in rsa p384 ed25519; do
    server_case "$k certificate" leaf_$k.pem leaf_$k.key
done
cat leaf_d2.pem chain_d2.pem > full_d2.pem
server_case "chain with an intermediate" full_d2.pem leaf_d2.key

# curl against std::http's server.
echo "== curl against std::http server"
: > http_server.out
"$W/http_server" 9 > http_server.out 2>&1 &
HPID=$!
for _ in $(seq 100); do sleep 0.1; grep -q '^port' http_server.out && break; done
HP=$(sed -n 's/^port //p' http_server.out)
U="http://127.0.0.1:$HP"
[ "$(curl -s -m 20 "$U/hello")" = "hello" ] && ok "curl GET" || bad "curl GET"
code=$(curl -s -m 20 -o /dev/null -w '%{http_code} %{size_download}' -I "$U/hello"); [ "$code" = "200 0" ] && ok "curl HEAD ($code)" || bad "curl HEAD: $code"
both=$(curl -s -m 20 -v "$U/hello" "$U/hello" 2>&1); n=$(grep -c '^hello$' <<<"$both"); grep -q -i 'Re-using existing connection\|Reusing existing connection' <<<"$both" && [ "$n" = 2 ] && ok "curl keep-alive, two requests on one connection" || bad "curl keep-alive (answers $n)"
size=$(curl -s -m 20 -o /dev/null -w '%{size_download}' "$U/big"); [ "$size" = 100000 ] && ok "curl large body ($size)" || bad "curl large body: $size"
head -c 70000 /dev/urandom > upload.bin
got=$(curl -s -m 20 -H 'Transfer-Encoding: chunked' --data-binary @upload.bin "$U/echo"); [ "$got" = 70000 ] && ok "curl chunked upload ($got bytes)" || bad "curl chunked upload: $got"
big=$(head -c 6000 /dev/zero | tr '\0' 'x')
got=$(curl -s -m 20 -H "X-Big: $big" "$U/headers"); [ "${got:-0}" -ge 6000 ] 2>/dev/null && ok "curl large headers ($got bytes)" || bad "curl large headers: $got"
code=$(curl -s -m 20 -o /dev/null -w '%{http_code}' "$U/missing"); [ "$code" = 404 ] && ok "curl 404" || bad "curl 404: $code"
kill $HPID 2>/dev/null; wait $HPID 2>/dev/null

# std::http's client against python's servers.
echo "== std::http client against python3"
mkdir -p www; head -c 12345 /dev/zero > www/file.bin; echo hi > www/small.txt
PP=$(port)
(cd www && exec python3 -m http.server --bind 127.0.0.1 "$PP" > ../py.log 2>&1) &
PYPID=$!
for _ in $(seq 50); do sleep 0.1; (exec 3<>/dev/tcp/127.0.0.1/$PP) 2>/dev/null && break; done
out=$(timeout -k 5 60 "$W/http_client" "http://127.0.0.1:$PP/file.bin" "http://127.0.0.1:$PP/small.txt" "http://127.0.0.1:$PP/missing")
[ "$(echo "$out" | sed -n 1p)" = "200 12345" ] && ok "client GET (python http.server)" || bad "client GET: $out"
[ "$(echo "$out" | sed -n 2p)" = "200 3" ] && ok "client second GET" || bad "client second GET: $out"
[ "$(echo "$out" | sed -n 3p)" = "404 $(echo "$out" | sed -n 3p | cut -d' ' -f2)" ] && [ "$(echo "$out" | sed -n 3p | cut -d' ' -f1)" = 404 ] && ok "client 404" || bad "client 404: $out"
kill $PYPID 2>/dev/null; wait $PYPID 2>/dev/null
CP=$(port)
cat > chunked.py <<'EOF'
import http.server, sys
class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def do_GET(self):
        self.send_response(200)
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()
        for part in (b"abc", b"x" * 5000, b"", ):
            if part:
                self.wfile.write(b"%x\r\n%s\r\n" % (len(part), part))
        self.wfile.write(b"0\r\n\r\n")
    def log_message(self, *a):
        pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
EOF
python3 chunked.py "$CP" &
CPID=$!
for _ in $(seq 50); do sleep 0.1; (exec 3<>/dev/tcp/127.0.0.1/$CP) 2>/dev/null && break; done
out=$(timeout -k 5 60 "$W/http_client" "http://127.0.0.1:$CP/a" "http://127.0.0.1:$CP/b")
[ "$out" = "$(printf '200 5003\n200 5003')" ] && ok "client chunked answer, kept alive" || bad "client chunked: $out"
kill $CPID 2>/dev/null; wait $CPID 2>/dev/null

echo "interop: $PASS passed, $FAIL failed"
[ $FAIL -eq 0 ]
