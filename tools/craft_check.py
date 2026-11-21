#!/usr/bin/env python3
"""Craft every recipe this package carries, in a real crafting table.

Called by tools/e2e.sh with the open game's control socket and a crafting
table's position. The harness's crafting grid is `ctl screen`: slots 100-108
row-major, result 200. For each carried recipe this asserts the output **id and
count**, that the crafted stack reaches the inventory, and that the ingredients
were consumed. Two controls then prove the grid is not passing everything:
a single comparator, and the prediction-matrix pattern with its clay ball
missing, both produce nothing.

Exits 1 with the observed result on the first mismatch.
"""
import json
import subprocess
import sys

# recipe file -> (output, count, {(row, column): item})
RECIPES = [
    (
        "sim_chamber.json",
        "hostilenetworks:sim_chamber",
        1,
        {
            (0, 1): "minecraft:glass_pane",
            (1, 0): "minecraft:ender_pearl",
            (1, 1): "minecraft:obsidian",
            (1, 2): "minecraft:ender_pearl",
            (2, 0): "minecraft:lapis_lazuli",
            (2, 1): "minecraft:comparator",
            (2, 2): "minecraft:lapis_lazuli",
        },
    ),
    (
        "loot_fabricator.json",
        "hostilenetworks:loot_fabricator",
        1,
        {
            (0, 1): "minecraft:netherite_ingot",
            (1, 0): "minecraft:diamond",
            (1, 1): "minecraft:obsidian",
            (1, 2): "minecraft:diamond",
            (2, 0): "minecraft:gold_ingot",
            (2, 1): "minecraft:comparator",
            (2, 2): "minecraft:gold_ingot",
        },
    ),
    (
        "prediction_matrix.json",
        "hostilenetworks:prediction_matrix",
        16,
        {
            (0, 0): "minecraft:iron_ingot",
            (0, 1): "minecraft:glass_pane",
            (1, 0): "minecraft:glass_pane",
            (1, 1): "minecraft:clay_ball",
            (1, 2): "minecraft:glass_pane",
            (2, 1): "minecraft:glass_pane",
            (2, 2): "minecraft:gold_ingot",
        },
    ),
]


def main():
    sock, ctl, x, y, z = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5]

    def run(*args):
        out = subprocess.run([ctl, "--socket", sock, *args], text=True, capture_output=True)
        try:
            return json.loads(out.stdout)
        except json.JSONDecodeError:
            raise SystemExit(f"ctl {' '.join(args)}: {out.stdout}{out.stderr}")

    def wait(ticks=4):
        run("wait", str(ticks))

    def slots():
        screen = run("screen")
        if "slots" not in screen:
            raise SystemExit(f"no menu is open: {screen}")
        return screen["slots"]

    def find(item):
        for slot in slots():
            if slot.get("item") == item:
                return slot["slot"]
        raise SystemExit(f"missing ingredient {item} in the inventory")

    def result():
        stack = [s for s in slots() if s["slot"] == 200]
        if not stack or not stack[0].get("item"):
            return None
        return stack[0]

    def place(item, targets):
        slot = find(item)
        run("click-slot", str(slot))
        wait(1)
        for target in targets:
            run("click-slot", str(target), "right")
            wait(1)
        run("click-slot", str(slot))
        wait(2)

    def grid(cells):
        """cells: {(row, column): item} -> the grid's slot numbers."""
        return {100 + row * 3 + column: item for (row, column), item in cells.items()}

    def ingredients(cells):
        counts = {}
        for item in cells.values():
            counts[item] = counts.get(item, 0) + 1
        return sorted(counts.items())

    def setup(items):
        try:
            run("close")
        except SystemExit:
            pass
        wait(5)
        # Replace the table so its block entity, and therefore the crafting
        # grid, is fresh for this case.
        run("setblock", x, y, z, "minecraft:air")
        run("setblock", x, y, z, "minecraft:crafting_table")
        wait(2)
        run("command", f"tp @s {float(x) - 0.5} {y} {float(z) + 0.5}")
        wait(3)
        run("command", "clear @s")
        for item, count in items:
            run("give", item, str(count))
        wait(3)
        wait()
        print("debug interact:", run("interact", x, y, z))
        wait(5)

    failures = []

    for name, output, count, cells in RECIPES:
        target = grid(cells)
        setup(ingredients(cells))
        for item in dict.fromkeys(cells.values()):
            place(item, [str(slot) for slot, at in target.items() if at == item])
        got = result()
        print(f"{name}: {dict(ingredients(cells))} -> {json.dumps(got)}")
        if not got or got.get("item") != output or got.get("count") != count:
            failures.append(f"{name} crafted {got}, expected {count} x {output}")
            continue
        run("click-slot", "200")
        wait(5)
        run("click-slot", "9")
        wait(3)
        counts = run("inventory")["counts"]
        made = counts.get(output, 0)
        left = sum(counts.get(item, 0) for item in set(cells.values()))
        print(f"  inventory: {made} x {output}, {left} ingredients left")
        if made < count:
            failures.append(f"{name}'s output reached the inventory as {made}, not {count}")
        if left:
            failures.append(f"{name} did not consume its ingredients ({left} left)")

    # Control 1: one comparator is not a machine.
    setup([("minecraft:comparator", 1)])
    place("minecraft:comparator", ["104"])
    got = result()
    print("comparator alone ->", json.dumps(got))
    if got is not None:
        failures.append(f"a comparator alone crafted {got['item']} x{got['count']}")

    # Control 2: the prediction-matrix pattern without its clay ball.
    cells = dict(RECIPES[2][3])
    del cells[(1, 1)]
    target = grid(cells)
    setup(ingredients(cells))
    for item in dict.fromkeys(cells.values()):
        place(item, [str(slot) for slot, at in target.items() if at == item])
    got = result()
    print("prediction matrix without the clay ball ->", json.dumps(got))
    if got is not None:
        failures.append(f"the matrix pattern without clay crafted {got['item']} x{got['count']}")

    run("close")
    wait(3)
    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        raise SystemExit(1)
    print(
        "recipe PASS: all three carried recipes craft their declared output and count "
        "and consume their ingredients; comparator-alone and missing-clay controls stay empty"
    )


if __name__ == "__main__":
    main()
