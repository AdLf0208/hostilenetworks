#!/usr/bin/env bash
# Proves the built content package (content/hostilenetworks.hmod) loads on a
# real game: the shared, unmodded game for this pin with `--dev-mod <the
# .hmod>` must register both machines and the items. A second launch WITHOUT
# `--dev-mod` is the negative control: the same ids must be unknown there, so
# the check cannot pass vacuously.
set -euo pipefail
export LC_ALL=C

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
eval "$("$CLONE/ports/tools/shared-bin.sh" env)"
HMOD=${HMOD:-$CLONE/ports/hostilenetworks/content/hostilenetworks.hmod}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/hostilenetworks-hmod.sock}

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -f "$HMOD" ] || { echo "missing package: $HMOD" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/hostilenetworks-hmod-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 60 >&2 || true; exit 1; }

cleanup() {
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "${KEEP:-0}" = 1 ]; then echo "kept scratch: $SCRATCH" >&2; else rm -rf "$SCRATCH" 2>/dev/null || true; fi
}
trap cleanup EXIT

launch() { # launch WORLD [--dev-mod PATH]
    local world=$1; shift
    rm -f "$SOCK"
    cd "$SCRATCH"
    "$CTL" --socket "$SOCK" launch \
        --game "$GAME" --world "$world" --seed 20261105 --creative --headless --wait \
        --resources "$RES" "$@" >/dev/null
    ctl wait-ready 120
    ctl wait 20
}

echo "== launch WITH the package (--dev-mod $HMOD) =="
launch HostileNeuralNetworksHmod --dev-mod "$HMOD"
ctl command "setblock 0 64 0 hostilenetworks:sim_chamber[facing=north]" >/dev/null
ctl command "setblock 2 64 0 hostilenetworks:loot_fabricator[facing=north]" >/dev/null
ctl wait 4
chamber=$(ctl block 0 64 0 | jq -r '.name // .block // empty')
fabricator=$(ctl block 2 64 0 | jq -r '.name // .block // empty')
echo "chamber 0,64,0 = $chamber"
echo "fabricator 2,64,0 = $fabricator"
[ "$chamber" = "hostilenetworks:sim_chamber" ] || fail "the package did not register the chamber"
[ "$fabricator" = "hostilenetworks:loot_fabricator" ] || fail "the package did not register the fabricator"
ctl give hostilenetworks:data_model 1 >/dev/null
ctl give hostilenetworks:prediction_matrix 1 >/dev/null
ctl give hostilenetworks:prediction 1 >/dev/null
ctl wait 4
items=$(ctl inventory | jq -c '.counts | with_entries(select(.key | startswith("hostilenetworks:")))')
echo "items: $items"
jq -e 'has("hostilenetworks:data_model") and has("hostilenetworks:prediction_matrix") and has("hostilenetworks:prediction")' <<<"$items" >/dev/null \
    || fail "the package did not register all three items"
echo "PASS: the packaged .hmod loaded (blocks hostilenetworks:sim_chamber + loot_fabricator, items data_model/prediction_matrix/prediction)"

echo "== launch WITHOUT the package (negative control) =="
launch HostileNeuralNetworksNoMod
setblock=$(ctl command "setblock 0 64 0 hostilenetworks:sim_chamber" 2>&1 || true)
echo "setblock: $setblock"
give=$(ctl give hostilenetworks:data_model 1 2>&1 || true)
echo "give: $give"
if [ "$(ctl block 0 64 0 | jq -r '.name // .block // empty')" = "hostilenetworks:sim_chamber" ]; then
    fail "the chamber exists without --dev-mod: the load check is vacuous"
fi
grep -qi "unknown block\|unknown item" <<<"$setblock$give" \
    || fail "neither the block nor the item was refused without the mod: $setblock / $give"
echo "PASS: without --dev-mod the machines and items are unknown, so the load check is not vacuous"
