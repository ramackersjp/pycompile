import sys


def summarize(label, *values, **options):
    total = 0
    for v in values:
        total += v
    verbose = options.get("verbose", False)
    if verbose:
        print(f"{label}: {list(values)} -> sum={total}")
    else:
        print(f"{label}: sum={total}")


print("program arguments:", sys.argv[1:])
summarize("plain", 1, 2, 3)
summarize("loud", 10, 20, verbose=True)
