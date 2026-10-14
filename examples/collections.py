squares = [x * x for x in range(1, 11)]
print("squares:", squares)
print("evens:", [x for x in squares if x % 2 == 0])

mods = {x % 3 for x in range(10)}
print("mods:", sorted(mods))

index = {x: x * x for x in range(5)}
print("index:", index)

words = ["banana", "apple", "cherry", "apple"]
counts = {}
for w in words:
    counts[w] = counts.get(w, 0) + 1
print("counts:", counts)

pairs = list(zip(squares, words))
print("zipped:", pairs[:2])

matrix = [[1, 2], [3, 4], [5, 6]]
flat = []
for row in matrix:
    for n in row:
        flat.append(n)
print("flat:", flat)
