total = 0
for i in range(300000):
    row = [i, i + 1, i + 2, str(i)]
    total += row[0] % 10
print(total)

# A reference cycle is preserved as long as it is reachable.
cycle = []
cycle.append(cycle)
print(len(cycle), cycle[0] is cycle)


def make_adder(n):
    return lambda x: x + n


adders = []
for i in range(50000):
    adders.append(make_adder(i))
print(adders[0](100), adders[49999](100))


class Node:
    def __init__(self, value):
        self.value = value
        self.next = None


head = Node(0)
node = head
for i in range(1, 50000):
    node.next = Node(i)
    node = node.next

count = 0
node = head
while node is not None:
    count += 1
    node = node.next
print(count)
