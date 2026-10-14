text = """
the quick brown fox jumps over the lazy dog
the dog barks and the fox runs away
"""

counts = {}
for word in text.split():
    counts[word] = counts.get(word, 0) + 1


def by_count(pair):
    return pair[1]


ordered = sorted(counts.items(), key=by_count)
for word, n in ordered:
    print(f"{word:>8}: {n}")
