class Shape:
    kind = "shape"

    def __init__(self, name):
        self.name = name

    def area(self):
        return 0

    def __str__(self):
        return self.kind + ":" + self.name


class Circle(Shape):
    kind = "circle"

    def __init__(self, name, radius):
        self.name = name
        self.radius = radius

    def area(self):
        return 3 * self.radius * self.radius


class Rectangle(Shape):
    kind = "rect"

    def __init__(self, name, width, height):
        self.name = name
        self.width = width
        self.height = height

    def area(self):
        return self.width * self.height


shapes = [Circle("c1", 2), Rectangle("r1", 3, 4), Shape("base")]
for shape in shapes:
    print(shape, "area", shape.area())

print(Circle.kind, Rectangle.kind, Shape.kind)

# Instance attributes and class attributes
c = Circle("c2", 5)
c.kind = "big-circle"
print(c.kind, Circle.kind)

# Instances are usable as dict keys via identity and support isinstance
print(isinstance(c, Circle), isinstance(c, Shape), isinstance(c, Rectangle))


class Counter:
    total = 0

    def __init__(self):
        Counter.total += 1
        self.index = Counter.total

    def __repr__(self):
        return "Counter(" + str(self.index) + ")"


print(Counter(), Counter(), Counter())


class Bag:
    def __init__(self):
        self.items = []

    def add(self, value):
        self.items.append(value)
        return self

    def __len__(self):
        return len(self.items)

    def __getitem__(self, index):
        return self.items[index]

    def __contains__(self, value):
        return value in self.items


bag = Bag()
bag.add("a").add("b").add("c")
print(len(bag), bag[0], bag[-1], "b" in bag, "z" in bag)
