data = [42, 7, 19, 3, 88, 7, 1]
print("original:", data)

nums = list(data)
nums.sort()
print("sorted:", nums)

desc = list(data)
desc.sort(reverse=True)
print("desc:", desc)

print("builtin:", sorted(data))
print("reverse:", sorted(data, reverse=True))
print("min/max/sum:", min(data), max(data), sum(data))

words = ["pear", "fig", "banana", "kiwi"]
print("by length:", sorted(words, key=len))
