# Exception handling: try / except / else / finally, raise, assert


def safe_div(a, b):
    try:
        return a // b
    except ZeroDivisionError as e:
        print("cannot divide:", e)
        return None


print(safe_div(10, 2), safe_div(10, 0))


def lookup(d, key):
    try:
        return d[key]
    except KeyError as e:
        return "missing " + str(e)


print(lookup({"a": 1}, "a"), lookup({"a": 1}, "b"))


def classify(value):
    try:
        xs = [1, 2, 3]
        return xs[value]
    except IndexError:
        return "out of range"
    except TypeError:
        return "not an index"
    else:
        pass


print(classify(1), classify(9))


def cleanup(flag):
    log = []
    try:
        log.append("try")
        if flag:
            raise ValueError("raised")
        log.append("ok")
    except ValueError as e:
        log.append("caught " + str(e))
    finally:
        log.append("finally")
    return log


print(cleanup(False))
print(cleanup(True))


def reraise():
    try:
        raise RuntimeError("inner")
    except RuntimeError as e:
        print("handling", e)
        raise


try:
    reraise()
except RuntimeError as e:
    print("outer got", e)


def multi(kind):
    try:
        if kind == "value":
            raise ValueError("v")
        if kind == "type":
            raise TypeError("t")
        raise IndexError("i")
    except (ValueError, TypeError) as e:
        return "common " + str(e)
    except IndexError:
        return "index"


print(multi("value"), multi("type"), multi("other"))


def check(n):
    try:
        assert n > 0, "must be positive"
        return "ok"
    except AssertionError as e:
        return "assert " + str(e)


print(check(5), check(-1))
