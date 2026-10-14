text = "The quick brown fox"
print(text.upper())
print(text.lower())
print(text.title())
print(text.replace("quick", "slow"))
print(text.split())
print("-".join(text.split()))
print(text.startswith("The"), text.endswith("fox"))
print(text.find("brown"), text.count("o"))
print(text[4:9])
print(text[::-1])

for word in text.split():
    print(f"{word:>8} ({len(word)})")

pi = 3.14159
print(f"pi = {pi:.2f}, repr = {pi!r}")
