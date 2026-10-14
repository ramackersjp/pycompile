import math
from math import sqrt, pi, e


def distance(x1, y1, x2, y2):
    dx = x2 - x1
    dy = y2 - y1
    return sqrt(dx * dx + dy * dy)


print("pi =", round(pi, 6))
print("e  =", round(e, 6))
print("sqrt(2) =", round(sqrt(2), 6))
print("distance =", round(distance(0, 0, 3, 4), 6))
print("floor/ceil:", math.floor(3.7), math.ceil(3.2))
print("factorial(5) =", math.factorial(5))
print("gcd(48, 36) =", math.gcd(48, 36))
print("hypot(3, 4) =", math.hypot(3, 4))
print("log10(1000) =", round(math.log10(1000), 6))
