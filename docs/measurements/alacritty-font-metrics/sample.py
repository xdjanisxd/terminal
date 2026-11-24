"""Draw the 80-by-24 font comparison sample in a disposable terminal pane."""

import sys


def at(row, column, text):
    return f"\x1b[{row};{column}H{text}"


def sample():
    output = ["\x1b[?25l\x1b[2J"]
    for row, style in ((3, "0"), (5, "1"), (7, "3"), (9, "1;3")):
        output.append(at(row, 4, f"\x1b[{style}mAffix fjyWQ agpq 0123456789 [](){{}} @#%\x1b[0m"))

    box = (
        "┌────────────────────────┬─────┐",
        "│ │ │ │ │ │ │ │ │ │ │ │  │     │",
        "├────────────────────────┼─────┤",
        "│ │ │ │ │ │ │ │ │ │ │ │  │     │",
        "└────────────────────────┴─────┘",
    )
    output.extend(at(row, 4, line) for row, line in enumerate(box, 12))
    for row in (1, 24):
        output.append(at(row, 1, "\x1b[48;2;34;120;160m" + " " * 80 + "\x1b[0m"))
    return "".join(output)


if __name__ == "__main__":
    sys.stdout.write(sample())
    sys.stdout.flush()
    try:
        sys.stdin.readline()
    finally:
        sys.stdout.write("\x1b[?25h")
