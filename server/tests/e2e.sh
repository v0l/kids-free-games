#!/usr/bin/env bash
set -u
BASE="${BASE:-http://127.0.0.1:8799}"
fails=0
n=0

req() {
  local method=$1 path=$2 token=${3:-} data=${4:-}
  local args=(-s -o /tmp/e2e.body -w '%{http_code}' -X "$method" "$BASE$path")
  [ -n "$token" ] && args+=(-H "Authorization: Bearer $token")
  [ -n "$data" ] && args+=(-H 'Content-Type: application/json' -d "$data")
  STATUS=$(curl "${args[@]}")
  BODY=$(cat /tmp/e2e.body)
}

expect() {
  local what=$1 want_status=$2 jq_expr=${3:-} want=${4:-}
  n=$((n + 1))
  local got=""
  [ -n "$jq_expr" ] && got=$(echo "$BODY" | jq -r "$jq_expr" 2>/dev/null)
  if [ "$STATUS" != "$want_status" ] || { [ -n "$jq_expr" ] && [ "$got" != "$want" ]; }; then
    fails=$((fails + 1))
    echo "FAIL $what: status $STATUS (want $want_status) $jq_expr=$got (want $want)"
    echo "     $BODY"
  else
    echo "ok   $what"
  fi
}

NOW=$(date +%s)
SUFFIX=$RANDOM

req GET /v1/health; expect "health" 200 .ok true
req GET /v1/games; expect "games list" 200 '.games | length' 16
req GET /v1/me; expect "me without token" 401 .error unauthorized
req GET /v1/me "kgt_$(printf 'a%.0s' $(seq 64))"; expect "me with unknown token" 401 .error unauthorized
req POST /v1/players "" '{"name":"123"}'; expect "register bad name" 400 .error bad_request
req POST /v1/players "" '{"name":"mia","avatar":99}'; expect "register bad avatar" 400 .error bad_request
req POST /v1/players "" 'not json'; expect "register bad json" 400 .error bad_request

req POST /v1/players "" '{"name":"  mia  ","avatar":2,"device":"Pixel"}'
expect "register mia" 200 .player.name Mia
A=$(echo "$BODY" | jq -r .token)
A_ID=$(echo "$BODY" | jq -r .player.id)
req POST /v1/players "" '{"name":"leo","avatar":5}'
expect "register leo" 200 .player.avatar 5
B=$(echo "$BODY" | jq -r .token)
B_ID=$(echo "$BODY" | jq -r .player.id)

req GET /v1/me "$A"; expect "me" 200 .player.id "$A_ID"
expect "me playtime starts at zero" 200 .playtime.total_seconds 0
req PATCH /v1/me "$A" '{"name":"mimi","avatar":3}'; expect "rename" 200 .player.name Mimi
req PATCH /v1/me "$A" '{}'; expect "empty patch" 400 .error bad_request
req PATCH /v1/me "$A" '{"avatar":50}'; expect "patch bad avatar" 400 .error bad_request

req GET /v1/saves/profile "$A"; expect "no save yet" 404 .error not_found
req GET /v1/saves/bogus "$A"; expect "unknown slot" 404 .error not_found
req PUT /v1/saves/profile "$A" '{"data":"p.x.level=3\n","base_rev":0}'; expect "first save" 200 .rev 1
req PUT /v1/saves/profile "$A" '{"data":"other","base_rev":0}'; expect "create over existing conflicts" 409 .error rev_mismatch
expect "conflict reports current rev" 409 .rev 1
req PUT /v1/saves/profile "$A" '{"data":"p.x.level=4\n","base_rev":1}'; expect "update with right rev" 200 .rev 2
req PUT /v1/saves/profile "$A" '{"data":"stale","base_rev":1}'; expect "stale rev conflicts" 409 .rev 2
req PUT /v1/saves/snake "$A" '{"data":"{}","base_rev":5}'; expect "update missing save conflicts" 409 .rev 0
req GET /v1/saves/profile "$A"; expect "read save back" 200 '.data | rtrimstr("\n")' p.x.level=4
req GET /v1/saves/profile "$B"; expect "other player cannot see it" 404 .error not_found
req GET /v1/saves "$A"; expect "list saves" 200 '.saves | length' 1
BIG=$(head -c 70000 /dev/zero | tr '\0' a)
req PUT /v1/saves/profile "$A" "{\"data\":\"$BIG\",\"base_rev\":2}"; expect "oversize save" 413 .error too_large

req POST /v1/scores/nope "$A" '{"score":5}'; expect "score unknown game" 404 .error not_found
req POST /v1/scores/snake "$A" '{"score":0}'; expect "zero score" 400 .error bad_request
req POST /v1/scores/snake "$A" '{"score":10}'; expect "first score" 200 .rank 1
expect "first score is an improvement" 200 .improved true
req POST /v1/scores/snake "$B" '{"score":25}'; expect "leo takes the lead" 200 .rank 1
req POST /v1/scores/snake "$A" '{"score":4}'; expect "worse score keeps best" 200 .best 10
expect "worse score is not an improvement" 200 .improved false
expect "mia is second" 200 .rank 2
req POST /v1/scores/snake "$A" '{"score":30}'; expect "mia overtakes" 200 .rank 1
req GET /v1/scores/snake "$A"; expect "board top" 200 '.entries[0].name' Mimi
expect "board has two" 200 '.entries | length' 2
expect "board marks me" 200 '.entries[0].you' true
expect "board better" 200 .better higher
expect "my rank" 200 .me.rank 1
req GET "/v1/scores/snake?limit=1" "$B"; expect "limit respected" 200 '.entries | length' 1
expect "b sees own rank below the cut" 200 .me.rank 2
req GET /v1/scores/flappy "$A"; expect "empty board" 200 '.entries | length' 0
expect "no rank yet" 200 .me null

req POST /v1/scores/memory "$A" '{"score":20}'; expect "memory first" 200 .rank 1
req POST /v1/scores/memory "$A" '{"score":14}'; expect "fewer moves improves" 200 .improved true
expect "fewer moves is best" 200 .best 14
req POST /v1/scores/memory "$A" '{"score":30}'; expect "more moves keeps best" 200 .best 14
req POST /v1/scores/memory "$B" '{"score":14}'; expect "tie goes to the earlier" 200 .rank 2
req GET /v1/scores/memory "$B"; expect "lower is better board" 200 .better lower
expect "lowest first" 200 '.entries[0].score' 14

req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-aaaa-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW - 700)),\"seconds\":300},{\"id\":\"sess-bbbb-$SUFFIX\",\"game\":\"memory\",\"started_at\":$((NOW - 300)),\"seconds\":120}]}"
expect "playtime upload" 200 .applied 2
req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-aaaa-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW - 700)),\"seconds\":300}]}"
expect "playtime replay is idempotent" 200 .applied 0
req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-aaaa-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW - 700)),\"seconds\":360}]}"
expect "a longer replay extends the session" 200 .applied 1
req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-aaaa-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW - 700)),\"seconds\":10}]}"
expect "a shorter replay changes nothing" 200 .applied 0
req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-cccc-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW + 99999)),\"seconds\":10}]}"
expect "future session refused" 400 .error bad_request
req POST /v1/playtime "$A" "{\"sessions\":[{\"id\":\"sess-cccc-$SUFFIX\",\"game\":\"snake\",\"started_at\":$((NOW - 100)),\"seconds\":99999}]}"
expect "absurd length refused" 400 .error bad_request
req POST /v1/playtime "$A" "{\"sessions\":[]}"; expect "empty batch refused" 400 .error bad_request
req POST /v1/playtime "$B" "{\"sessions\":[{\"id\":\"sess-aaaa-$SUFFIX\",\"game\":\"flappy\",\"started_at\":$((NOW - 200)),\"seconds\":50}]}"
expect "same session id for another player is separate" 200 .applied 1
req GET /v1/playtime "$A"; expect "total playtime" 200 .total_seconds 480
expect "sessions counted" 200 .sessions 2
expect "top game" 200 '.by_game[0].game' snake
expect "snake seconds" 200 '.by_game[0].seconds' 360
expect "today bucket" 200 '.by_day[-1].seconds' 480
expect "today label" 200 '.by_day[-1].day' "$(date -u +%F)"
req GET /v1/me "$A"; expect "me shows playtime" 200 .playtime.total_seconds 480
req GET /v1/playtime "$B"; expect "other player total" 200 .total_seconds 50

req POST /v1/link "$A"; expect "link code" 200 '.code | length' 9
CODE=$(echo "$BODY" | jq -r .code)
req POST /v1/link/redeem "" '{"code":"ZZZZ-ZZZZ"}'; expect "wrong code" 404 .error not_found
req POST /v1/link/redeem "" '{"code":"nonsense"}'; expect "malformed code" 404 .error not_found
req POST /v1/link/redeem "" "{\"code\":\"$(echo "$CODE" | tr A-Z a-z)\",\"device\":\"iPad\"}"
expect "redeem lower case" 200 .player.id "$A_ID"
C=$(echo "$BODY" | jq -r .token)
req POST /v1/link/redeem "" "{\"code\":\"$CODE\"}"; expect "code is single use" 404 .error not_found
req GET /v1/saves/profile "$C"; expect "linked device sees the save" 200 .rev 2
req POST /v1/link "$A"; req POST /v1/link "$A"
OLD=$CODE; CODE=$(echo "$BODY" | jq -r .code)
req POST /v1/link/redeem "" "{\"code\":\"$OLD\"}"; expect "new code replaces the old" 404 .error not_found

req DELETE /v1/me "$B"; expect "delete leo" 200 .deleted true
req GET /v1/me "$B"; expect "deleted token dead" 401 .error unauthorized
req GET /v1/scores/snake "$A"; expect "deleted player leaves the board" 200 '.entries | length' 1
req GET /v1/nope "$A"; expect "unknown route" 404 .error not_found
req DELETE /v1/saves/profile "$A"; expect "wrong method" 405 .error method_not_allowed
req DELETE /v1/me "$A"; expect "delete mia" 200 .deleted true
req GET /v1/me "$C"; expect "linked token dies with the player" 401 .error unauthorized

echo
echo "$((n - fails))/$n passed"
[ "$fails" -eq 0 ]
