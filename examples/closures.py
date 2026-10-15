# Lambdas and closures


def make_adder(n):
    return lambda x: x + n


def make_counter(start):
    def next_value():
        return start + 1

    return next_value


def compose(f, g):
    return lambda x: f(g(x))


def outer(a):
    def middle(b):
        def inner(c):
            return a * 100 + b * 10 + c

        return inner

    return middle


print("adders:", make_adder(5)(3), make_adder(100)(3))
print("counter:", make_counter(41)())
print("compose:", compose(lambda x: x + 1, lambda x: x * 2)(10))

xs = [("b", 2), ("a", 3), ("c", 1)]
print("by key:", sorted(xs, key=lambda p: p[0]))
print("by value desc:", sorted(xs, key=lambda p: p[1], reverse=True))
print("numbers desc:", sorted([5, 2, 8, 1], key=lambda n: -n))

funcs = []
for i in range(3):
    funcs.append(lambda: i)
print("late binding:", funcs[0](), funcs[1](), funcs[2]())

base = 10
print("captures global:", (lambda x: x + base)(5))

f = lambda x, y=100: x + y
print("defaults:", f(1), f(1, 2))

print("chain:", outer(1)(2)(3))

apply = lambda fn, v: fn(v)
print("higher order:", apply(lambda x: x * x, 7))
