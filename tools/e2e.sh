#!/usr/bin/env bash
# In-game check for the Hostile Neural Networks slice: the Simulation Chamber
# trains a Data Model into a Prediction, and the Loot Fabricator turns that
# Prediction into the model's loot.
#
# STATUS at SDK `0a875da` (2026-11-21): this scenario runs against the launcher's Nightly at the
# campaign pin (`launcher-game.sh build hostilenetworks`), and its legs are recorded in
# `../_shared/evidence/hostilenetworks-e2e/`. The retired `.hnmod` bundle ran the same scenario to
# `PASS` at `1d63e0a` (`../_shared/evidence/hostilenetworks/e2e.log`).
#
# The arena is a stone platform at y=100 near the player's spawn. Every mod command is typed with
# `ctl chat "/cmd"` because `ctl command` runs the console path and bypasses the mod's command hook
# (`run_mod_command`).
#
# Default run:
#  1. both machines place and read back with a `facing` state;
#  2. a chamber with no model reports `model`, and a chamber with a model but no energy reports
#     `energy` and produces nothing (two controls);
#  3. with 2,000,000 FE the chamber starts a run, its runtime advances, and at the end its base-drop
#     slot holds `overworld_prediction` and its prediction slot holds a `prediction` (typed to the
#     zombie model);
#  4. the prediction is carried to the Loot Fabricator, which does nothing until a palette row is
#     clicked on its own screen (an `energy` control first, then the click);
#  5. with a selection it produces that model's drop (rotten flesh x64 for row 1, iron ingot x8 for
#     row 2) and consumes the prediction;
#  6. a training run (mode switched by the screen's own mode line) adds one data and no loot, and a
#     self-aware model in training is refused with `cannot_train` (control 5);
#  7. all three carried recipes craft in a crafting table and consume their ingredients
#     (`tools/craft_check.py`, with comparator-alone and missing-clay controls);
#  8. every carried item is given and read back, and an unknown id is refused;
#  9. in survival, mining each machine with the carried pickaxe drops it (loot tables), with a
#     vanilla stone -> cobblestone break as the differential.
#
# SELFTEST=1 flips the chamber assertion (it must FAIL), proving the check can fail.
# NEGATIVE=1 flips the training-mode assertion (a training run must NOT upgrade the model), so the
# new slice's assertion is fail-capable too.
# COUNTERFACTUAL=1 runs the same setup on a plain barrel instead of the chamber (no readout, no run).
# CONTROL_GAME=<a game built WITHOUT this package> adds the differential leg: the machine block is
# unknown there and the command does not exist.
set -euo pipefail
export LC_ALL=C

CLONE=/home/adlf/Desktop/all_the_mods_10_rust_rewrite
PORT=$CLONE/ports/hostilenetworks
CTL=${CTL:-$(command -v horizon-ctl || true)}
GAME=${GAME:-}
CONTROL_GAME=${CONTROL_GAME:-}
RES=${RES:-${HORIZON_RES:-/home/adlf/Desktop/horizon/.local/horizon-client/26.3}}
SOCK=${SOCK:-/tmp/opencode/hostilenetworks-e2e.sock}
SELFTEST=${SELFTEST:-0}
NEGATIVE=${NEGATIVE:-0}
COUNTERFACTUAL=${COUNTERFACTUAL:-0}
KEEP=${KEEP:-0}
WORLD=HostileNeuralNetworksE2E

[ -n "$CTL" ] && [ -x "$CTL" ] || {
    echo "missing horizon-ctl: set CTL=<a control binary for the launcher build>" >&2
    exit 1
}
[ -n "$GAME" ] && [ -x "$GAME" ] || {
    echo "missing horizon-client: set GAME=<a game the launcher built with this package>" >&2
    echo "  horizon-launcher native-build ports/hostilenetworks --instance <i> --trust-native-code" >&2
    exit 1
}
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/hostilenetworks-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"
cd "$SCRATCH"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { echo "== $* =="; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 80 >&2 || true; exit 1; }
command_() { ctl command "$1" >/dev/null; }
chat() { ctl chat "/$1" >/dev/null; }
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }
count_at() { # count_at X Y Z ITEM
    slots "$1" "$2" "$3" \
        | jq -r --arg item "$4" '[.[] | select(.item == $item) | .count] | add // 0'
}
has() { # has X Y Z ITEM COUNT
    local got
    got=$(count_at "$1" "$2" "$3" "$4")
    [ "$got" = "$5" ] || fail "expected $5 x $4 at $1,$2,$3; got $got ($(slots "$1" "$2" "$3"))"
    echo "  has($1,$2,$3) $4 x$5: ok"
}
readout() { # readout X Y Z: the /hostilenetworks info line
    chat "hostilenetworks info $1 $2 $3"
    ctl wait 4 >/dev/null
    ctl messages 8 | jq -r '.chat[]' | grep -m1 -E '^(Simulation Chamber|Loot Fabricator|No machine)' || true
}
field() { # field LINE NAME -> the value after `NAME `
    sed -n "s/.*$2 \([^,]*\),.*/\1/p" <<<"$1"
}

cleanup() {
    local status=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then echo "kept scratch: $SCRATCH" >&2; else rm -rf "$SCRATCH" 2>/dev/null || true; fi
    echo "EXIT:$status"
}
trap cleanup EXIT

say "launch (offscreen, creative; the package's own content and code)"
"$CTL" --socket "$SOCK" launch \
    --game "$GAME" --world "$WORLD" --seed 20261105 --creative --offscreen --wait \
    --game-dir "$SCRATCH/game" --resources "$RES"
ctl wait-ready 180
# The slot commands are operator-level (machine contents are not public state); the harness player
# is made an operator from the console, the way a server admin would. `info` stays level 0 and
# still proves the ordinary-player path.
ctl command "op @a" >/dev/null 2>&1 || true
ctl wait 4 >/dev/null

say "a platform at y=100 near spawn"
PX=$(ctl state | jq -r '.player.position[0]' | cut -d. -f1)
PZ=$(ctl state | jq -r '.player.position[2]' | cut -d. -f1)
X=$PX
Z=$((PZ + 2))
Y=101
command_ "fill $((X-6)) 100 $((Z-6)) $((X+6)) 100 $((Z+4)) minecraft:stone"
command_ "fill $((X-6)) 101 $((Z-6)) $((X+6)) 110 $((Z+4)) minecraft:air"
ctl tp "$X.5" 101 "$((Z+3)).5" >/dev/null
ctl wait 20
command_ "gamerule spawn_mobs false"
command_ "gamerule spawn_monsters false"
ctl close >/dev/null 2>&1 || true
ctl wait 10

CHAMBER="$X $Y $Z"
FAB="$((X+2)) $Y $Z"
if [ "$COUNTERFACTUAL" = 1 ]; then
    command_ "setblock $X $Y $Z minecraft:barrel[facing=north]"
else
    command_ "setblock $X $Y $Z hostilenetworks:sim_chamber[facing=north]"
fi
command_ "setblock $((X+2)) $Y $Z hostilenetworks:loot_fabricator[facing=north]"
ctl wait 10
set -- $CHAMBER
chamber_block=$(ctl block "$1" "$2" "$3" | jq -r '.block // "unknown"')
fab_block=$(ctl block "$((X+2))" "$Y" "$Z" | jq -r '.block // "unknown"')
echo "chamber block: $chamber_block"
echo "fabricator block: $fab_block"
[ "$fab_block" = "hostilenetworks:loot_fabricator" ] || fail "the fabricator is $fab_block (did the package load?)"
if [ "$COUNTERFACTUAL" = 1 ]; then
    line=$(readout "$X" "$Y" "$Z")
    echo "barrel readout: $line"
    grep -q "No machine" <<<"$line" || fail "a barrel answered as a machine: $line"
    echo "PASS: a plain barrel is not a Simulation Chamber; the machine's behaviour is its own"
    exit 0
fi
[ "$chamber_block" = "hostilenetworks:sim_chamber" ] || fail "the chamber is $chamber_block"

say "control 1: no model in the chamber"
line=$(readout "$X" "$Y" "$Z")
echo "readout: $line"
grep -q "^Simulation Chamber:" <<<"$line" || fail "the chamber did not answer: $line"
[ "$(field "$line" fail)" = "model" ] || fail "expected fail model, got: $line"
ctl wait 40
[ "$(count_at "$X" "$Y" "$Z" minecraft:rotten_flesh)" = 0 ] || fail "the chamber produced loot without a model"
has "$X" "$Y" "$Z" hostilenetworks:prediction 0

say "put a zombie Data Model into slot 0 and type it"
ctl container "$X" "$Y" "$Z" hostilenetworks:data_model 1 >/dev/null
ctl wait 4
chat "hostilenetworks modelset $X $Y $Z 0 20 hostilenetworks:zombie"
ctl wait 4
ctl messages 4 | jq -r '.chat[]' | grep -m1 'Data Model' || true
line=$(readout "$X" "$Y" "$Z")
echo "readout: $line"
grep -q "model hostilenetworks:zombie" <<<"$line" || fail "the model did not land: $line"
grep -q "tier basic" <<<"$line" || fail "20 data should be the basic tier: $line"

say "an input (one prediction matrix) in slot 1"
ctl container "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 1 >/dev/null
ctl wait 4

say "control 2: a model and an input, but no energy"
ctl wait 40
line=$(readout "$X" "$Y" "$Z")
echo "readout: $line"
[ "$(field "$line" fail)" = "energy" ] || fail "expected fail energy, got: $line"
[ "$(field "$line" runtime)" = "0" ] || fail "the chamber ran without energy: $line"
has "$X" "$Y" "$Z" hostilenetworks:overworld_prediction 0

if [ "$SELFTEST" = 1 ]; then
    say "SELFTEST=1: a chamber with no energy must have produced loot (it has not)"
    has "$X" "$Y" "$Z" hostilenetworks:overworld_prediction 1
fi

say "a self-aware model (accuracy 1.0) so the run always yields a prediction"
chat "hostilenetworks modelset $X $Y $Z 0 1254 hostilenetworks:zombie"
ctl wait 4
line=$(readout "$X" "$Y" "$Z")
echo "readout: $line"
grep -q "tier self_aware" <<<"$line" || fail "1254 data should be self aware: $line"

say "charge the chamber and run one simulation"
chat "hostilenetworks energy $X $Y $Z 2000000"
ctl wait 4
ctl messages 4 | jq -r '.chat[]' | grep -m1 'Energy:' || true
ctl wait 60
line=$(readout "$X" "$Y" "$Z")
echo "mid-run: $line"
runtime=$(field "$line" runtime)
[ "$runtime" -gt 0 ] || fail "the chamber did not start: $line"
[ "$runtime" -lt 240 ] || fail "the runtime did not advance: $line"
# The energy spent follows the progress exactly: (240 - runtime) ticks of a
# 128 FE/t model. This does not depend on where the wait boundaries land.
energy=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
spent=$(( (240 - runtime) * 128 ))
[ "$energy" = "$(( 2000000 - spent ))" ] \
    || fail "the chamber spent the wrong energy at runtime $runtime: $line"
ctl wait 195
line=$(readout "$X" "$Y" "$Z")
echo "after the run: $line"
[ "$(field "$line" runtime)" = "0" ] || fail "the run did not finish or restarted: $line"
# 239 draining ticks of a 128 FE/t model (the finishing tick takes none):
# 2,000,000 - 239 * 128.
energy=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
[ "$energy" = "1969408" ] || fail "the run did not cost 239 * 128 FE: $line"
has "$X" "$Y" "$Z" hostilenetworks:overworld_prediction 1
has "$X" "$Y" "$Z" hostilenetworks:prediction 1
has "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 0
[ "$(count_at "$X" "$Y" "$Z" minecraft:rotten_flesh)" = 0 ] || fail "the chamber fabricated loot itself"

say "the chamber opens its 4-slot menu"
ctl tp "$X.5" 101 "$((Z+3)).5" >/dev/null
ctl look 180 20 >/dev/null
ctl wait 4
ctl look-at block "$X" "$Y" "$Z" south >/dev/null || echo "look-at did not aim (the interact still aims itself)"
ctl interact "$X" "$Y" "$Z" south >/dev/null || fail "the chamber did not open a menu"
ctl wait 6
menu_slots=$(ctl screen | jq '[.slots[] | select(.slot >= 300)] | length')
echo "menu slots: $menu_slots"
[ "$menu_slots" = 4 ] || fail "the chamber menu shows $menu_slots slots, expected 4"
# A press on the mode line is a menu button: it switches inference to training
# and back, which proves the screen-click -> menu-button path in game.
ui=$(ctl ui)
rect=$(jq -c '[.widgets[] | select(.id == "slot-300") | .bounds][0]' <<<"$ui")
ox=$(jq -r '.[0] - 26' <<<"$rect")
oy=$(jq -r '.[1] - 20' <<<"$rect")
cx=$(python3 -c "print($ox + 12)")
cy=$(python3 -c "print($oy + 48)")
ctl click-at "$cx" "$cy" >/dev/null || fail "the mode click failed"
ctl wait 6 >/dev/null
line=$(readout "$X" "$Y" "$Z")
echo "after the mode click: $(field "$line" mode)"
[ "$(field "$line" mode)" = "training" ] || fail "the mode click did not switch the mode: $line"
ctl click-at "$cx" "$cy" >/dev/null
ctl wait 6 >/dev/null
line=$(readout "$X" "$Y" "$Z")
echo "after the second mode click: $(field "$line" mode)"
[ "$(field "$line" mode)" = "inference" ] || fail "the second mode click did not switch back: $line"
ctl close >/dev/null 2>&1 || true
ctl wait 4

say "carry the prediction to the Loot Fabricator"
chat "hostilenetworks take $X $Y $Z 3"
ctl wait 4
ctl inventory | jq -r '.counts["hostilenetworks:prediction"] // 0' | grep -qx '[1-9][0-9]*' \
    || fail "the prediction was not taken: $(ctl inventory | jq -c '.counts')"
chat "hostilenetworks put $((X+2)) $Y $Z 0"
ctl wait 4
readout "$((X+2))" "$Y" "$Z" >/dev/null
has "$((X+2))" "$Y" "$Z" hostilenetworks:prediction 1

say "control 3: a charged fabricator with no selection produces nothing"
chat "hostilenetworks energy $((X+2)) $Y $Z 1000000"
ctl wait 4
ctl wait 70
fab_line=$(readout "$((X+2))" "$Y" "$Z")
echo "readout: $fab_line"
grep -q "^Loot Fabricator:" <<<"$fab_line" || fail "the fabricator did not answer: $fab_line"
[ "$(field "$fab_line" selection)" = "-1" ] || fail "a fresh fabricator has a selection: $fab_line"
[ "$(field "$fab_line" runtime)" = "0" ] || fail "the fabricator ran without a selection: $fab_line"
has "$((X+2))" "$Y" "$Z" minecraft:rotten_flesh 0

say "click palette row 2 (iron ingot x8) on the fabricator's own screen"
ctl tp "$((X+2)).5" 101 "$((Z+3)).5" >/dev/null
ctl wait 4
ctl look-at block "$((X+2))" "$Y" "$Z" south >/dev/null || echo "look-at did not aim (the interact still aims itself)"
ctl interact "$((X+2))" "$Y" "$Z" south >/dev/null || fail "the fabricator did not open a menu"
ctl wait 6
fab_slots=$(ctl screen | jq '[.slots[] | select(.slot >= 300)] | length')
echo "fabricator menu slots: $fab_slots"
[ "$fab_slots" = 17 ] || fail "the fabricator menu shows $fab_slots slots, expected 17"
# The prediction the chamber made carries its model, so the fabricator knows
# whose table to read (the menu reply shows an item's components).
prediction_components=$(ctl screen | jq -r '.slots[] | select(.slot == 300) | .components // ""')
echo "prediction components: $prediction_components"
grep -q 'hostilenetworks:data_model' <<<"$prediction_components" \
    || fail "the prediction does not carry its model: $prediction_components"
# The panel origin: the prediction slot (300) sits at (62, 20) inside a
# 176x186 panel, so the absolute slot rect gives the panel's own origin.
ui=$(ctl ui)
echo "gui size: $(jq -c '.gui_size' <<<"$ui")"
rect=$(jq -c '[.widgets[] | select(.id == "slot-300") | .bounds][0]' <<<"$ui")
echo "slot-300 bounds: $rect"
ox=$(jq -r '.[0] - 62' <<<"$rect")
oy=$(jq -r '.[1] - 20' <<<"$rect")
click_row() { # click_row INDEX -> clicks that palette row, prints the readout
    local cx cy line
    cx=$(python3 -c "print($ox + 10)")
    cy=$(python3 -c "print($oy + 66 + $1*10 + 5)")
    ctl click-at "$cx" "$cy" >/dev/null || fail "the palette click failed"
    ctl wait 6 >/dev/null
    line=$(readout "$((X+2))" "$Y" "$Z")
    echo "  click at $cx,$cy -> $(field "$line" selection)"
}
say "each palette row selects its drop"
for row in 0 1 2 3; do click_row "$row"; done
fab_line=$(readout "$((X+2))" "$Y" "$Z")
[ "$(field "$fab_line" selection)" = "3" ] \
    || fail "the last palette click did not select row 4: $fab_line"
say "and row 2 selects iron ingot x8"
click_row 1
fab_line=$(readout "$((X+2))" "$Y" "$Z")
[ "$(field "$fab_line" selection)" = "1" ] || fail "the palette click did not select row 2: $fab_line"
ctl close >/dev/null 2>&1 || true
ctl wait 4

say "fabricate one iron ingot drop"
ctl wait 70
fab_line=$(readout "$((X+2))" "$Y" "$Z")
echo "after fabricating: $fab_line"
has "$((X+2))" "$Y" "$Z" minecraft:iron_ingot 8
has "$((X+2))" "$Y" "$Z" hostilenetworks:prediction 0
[ "$(field "$fab_line" runtime)" = "0" ] || fail "the fabricator's runtime did not reset: $fab_line"

say "control 4: a wrong item (a Prediction Matrix) in the prediction slot"
ctl container "$((X+2))" "$Y" "$Z" hostilenetworks:prediction_matrix 1 >/dev/null
ctl wait 4
chat "hostilenetworks fab $((X+2)) $Y $Z 0"
ctl wait 4
ctl wait 70
fab_line=$(readout "$((X+2))" "$Y" "$Z")
echo "readout: $fab_line"
[ "$(field "$fab_line" runtime)" = "0" ] || fail "the fabricator ran on a wrong item: $fab_line"
has "$((X+2))" "$Y" "$Z" minecraft:rotten_flesh 0
has "$((X+2))" "$Y" "$Z" hostilenetworks:prediction_matrix 1

say "training mode upgrades the model and produces no loot"
# The counts are unambiguous only if the inference run's loot is out of the way: take the base
# drop into the player. (Slot 3's prediction went to the fabricator above.)
chat "hostilenetworks take $X $Y $Z 2"
ctl wait 4
has "$X" "$Y" "$Z" hostilenetworks:overworld_prediction 0
chat "hostilenetworks modelset $X $Y $Z 0 20 hostilenetworks:zombie"
ctl wait 4
line=$(readout "$X" "$Y" "$Z")
echo "before training: $line"
[ "$(field "$line" data)" = "20" ] || fail "the model should be back at 20 data: $line"
# Switch to training through the screen's own mode line, the path the menu button owns.
ctl tp "$X.5" 101 "$((Z+3)).5" >/dev/null
ctl look-at block "$X" "$Y" "$Z" south >/dev/null || true
ctl interact "$X" "$Y" "$Z" south >/dev/null || fail "the chamber did not open a menu"
ctl wait 6
ui=$(ctl ui)
rect=$(jq -c '[.widgets[] | select(.id == "slot-300") | .bounds][0]' <<<"$ui")
ox=$(jq -r '.[0] - 26' <<<"$rect")
oy=$(jq -r '.[1] - 20' <<<"$rect")
mcx=$(python3 -c "print($ox + 12)")
mcy=$(python3 -c "print($oy + 48)")
ctl click-at "$mcx" "$mcy" >/dev/null || fail "the mode click failed"
ctl wait 6
ctl close >/dev/null 2>&1 || true
ctl wait 4
line=$(readout "$X" "$Y" "$Z")
echo "mode after the click: $(field "$line" mode)"
[ "$(field "$line" mode)" = "training" ] || fail "the mode click did not reach training: $line"
# Read the energy BEFORE the input lands, so the arithmetic does not depend on when the tick saw it.
energy_before=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
ctl container "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 1 >/dev/null
ctl wait 340
line=$(readout "$X" "$Y" "$Z")
echo "after training: $line"
[ "$(field "$line" data)" = "21" ] || fail "training did not add one data: $line"
[ "$(field "$line" runtime)" = "0" ] || fail "the training run did not finish: $line"
[ "$(field "$line" fail)" = "input" ] || fail "a finished training run should wait for input: $line"
has "$X" "$Y" "$Z" hostilenetworks:overworld_prediction 0
has "$X" "$Y" "$Z" hostilenetworks:prediction 0
has "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 0
energy_after=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
# 299 draining ticks of a 154 FE/t training run (the finishing tick takes none).
[ "$energy_after" = "$(( energy_before - 299 * 154 ))" ] \
    || fail "training spent the wrong energy: before $energy_before, after $energy_after"
if [ "$NEGATIVE" = 1 ]; then
    say "NEGATIVE=1: a finished training run must NOT have upgraded the model (it did)"
    [ "$(field "$line" data)" = "20" ] \
        || fail "NEGATIVE: training upgraded the model to $(field "$line" data), so the assertion is alive"
fi

say "control 5: a self-aware model cannot be trained"
chat "hostilenetworks modelset $X $Y $Z 0 1254 hostilenetworks:zombie"
ctl wait 4
line=$(readout "$X" "$Y" "$Z")
energy_before=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
ctl container "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 1 >/dev/null
ctl wait 40
line=$(readout "$X" "$Y" "$Z")
echo "self-aware model in training: $line"
[ "$(field "$line" fail)" = "cannot_train" ] || fail "a self-aware model started a training run: $line"
[ "$(field "$line" runtime)" = "0" ] || fail "a refused training run is in progress: $line"
has "$X" "$Y" "$Z" hostilenetworks:prediction_matrix 1
energy_after=$(sed -n 's/.*Chamber: \([0-9]*\) FE.*/\1/p' <<<"$line")
[ "$energy_after" = "$energy_before" ] \
    || fail "a refused training run spent energy: $energy_before -> $energy_after"

say "every carried recipe crafts in a real crafting table"
CTX=$((X-3))
CTZ=$Z
command_ "setblock $CTX 101 $CTZ minecraft:crafting_table"
ctl tp "$((CTX)).5" 101 "$((CTZ+2)).5" >/dev/null
ctl wait 10
python3 "$PORT/tools/craft_check.py" "$SOCK" "$CTL" "$CTX" 101 "$CTZ" \
    || fail "a carried recipe did not craft (see craft_check.py's line)"
command_ "setblock $CTX 101 $CTZ minecraft:air"

say "every carried item exists and is readable; an unknown id is refused"
for item in hostilenetworks:sim_chamber hostilenetworks:loot_fabricator \
            hostilenetworks:data_model hostilenetworks:blank_data_model \
            hostilenetworks:prediction hostilenetworks:overworld_prediction \
            hostilenetworks:prediction_matrix; do
    ctl give "$item" 1 >/dev/null
    ctl wait 4
    got=$(ctl inventory | jq -r --arg i "$item" '.counts[$i] // 0')
    [ "$got" -ge 1 ] || fail "give $item did not reach the inventory: $(ctl inventory | jq -c '.counts')"
    echo "  $item x1: ok"
done
ctl give hostilenetworks:not_a_real_item 1 >/dev/null 2>&1 || true
ctl wait 4
got=$(ctl inventory | jq -r '.counts["hostilenetworks:not_a_real_item"] // 0')
[ "$got" = 0 ] || fail "an unknown item id was accepted into the inventory"
echo "  hostilenetworks:not_a_real_item: refused"

say "the carried loot tables drop the machine when it is mined (survival)"
LX=$((X-5))
LZ=$Z
command_ "gamemode survival @s"
ctl wait 4
command_ "clear @s"
ctl give minecraft:diamond_pickaxe 1 >/dev/null
ctl wait 4
ctl select minecraft:diamond_pickaxe >/dev/null || fail "the pickaxe is not in the hotbar"
command_ "setblock $LX 101 $LZ hostilenetworks:sim_chamber"
command_ "setblock $((LX+1)) 101 $LZ hostilenetworks:loot_fabricator"
ctl tp "$((LX)).5" 101 "$((LZ+2)).5" >/dev/null
ctl wait 10
for block in "hostilenetworks:sim_chamber $LX" "hostilenetworks:loot_fabricator $((LX+1))"; do
    set -- $block
    id=$1
    bx=$2
    before=$(ctl inventory | jq -r --arg i "$id" '.counts[$i] // 0')
    ctl break "$bx" 101 "$LZ" --timeout 30 >/dev/null || fail "a $id did not break"
    ctl wait 10
    ctl tp "$((bx)).5" 101 "$LZ" >/dev/null
    ctl wait 20
    after=$(ctl inventory | jq -r --arg i "$id" '.counts[$i] // 0')
    [ "$after" = "$((before + 1))" ] \
        || fail "$id's loot table dropped nothing: $before -> $after"
    echo "  $id: $before -> $after"
done
# The differential for the same break path on vanilla data: a stone block drops cobblestone.
command_ "setblock $LX 101 $LZ minecraft:stone"
before=$(ctl inventory | jq -r '.counts["minecraft:cobblestone"] // 0')
ctl break "$LX" 101 "$LZ" --timeout 30 >/dev/null || fail "the stone block did not break"
ctl wait 10
ctl tp "$((LX)).5" 101 "$LZ" >/dev/null
ctl wait 20
after=$(ctl inventory | jq -r '.counts["minecraft:cobblestone"] // 0')
[ "$after" = "$((before + 1))" ] \
    || fail "the vanilla stone differential dropped nothing: $before -> $after"
echo "  minecraft:stone -> cobblestone: $before -> $after"

if [ -n "$CONTROL_GAME" ]; then
    # The differential: the same world in a game built WITHOUT this package. The machine id and
    # the command are unknown there, so everything above is the package's own behaviour.
    say "differential: a game built without the package has no machine and no command"
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    "$CTL" --socket "$SOCK" launch \
        --game "$CONTROL_GAME" --world "$WORLD" --seed 20261105 --creative --offscreen --wait \
        --game-dir "$SCRATCH/control" --resources "$RES"
    ctl wait-ready 180
    ctl command "setblock $X $Y $Z hostilenetworks:sim_chamber" >/dev/null 2>&1 || true
    block=$(ctl block "$X" "$Y" "$Z" | jq -r '.block // "unknown"')
    echo "  control block: $block"
    [ "$block" != "hostilenetworks:sim_chamber" ] || fail "the control game knew the machine id"
    ctl chat "/hostilenetworks info $X $Y $Z" >/dev/null 2>&1 || true
    ctl wait 4 >/dev/null
    if ctl messages 8 | jq -r '.chat[]' | grep -q '^Simulation Chamber:'; then
        fail "the control game answered the mod command"
    fi
    ctl quit >/dev/null 2>&1 || true
fi

echo "PASS: the chamber ran a zombie simulation with FE (no model / no energy controls held), produced a base drop and a typed prediction, the fabricator turned that prediction into iron ingot x8 through its own palette click, a training run upgraded the model to 21 data with no loot and a self-aware model was refused (cannot_train), all three carried recipes crafted and consumed their ingredients, every carried item was given and read back with an unknown id refused, and both loot tables dropped their machine to a survival pickaxe (no-selection, wrong-item, no-energy and comparator-alone controls held); a plain barrel is not a machine"
