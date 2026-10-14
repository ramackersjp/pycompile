/* pyrt.c - runtime support library for programs compiled by pycompile. */
#include "pyrt.h"

#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <ctype.h>
#include <math.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
#ifndef M_E
#define M_E 2.7182818284590452354
#endif

enum {
    PY_NONE = 0,
    PY_BOOL,
    PY_INT,
    PY_FLOAT,
    PY_STR,
    PY_LIST,
    PY_TUPLE,
    PY_DICT,
    PY_SET,
    PY_RANGE,
    PY_FUNC,
    PY_ITER
};

struct Pyo {
    int kind;
    union {
        int b;
        long long i;
        double f;
        struct {
            char* s;
            long long len;
        } str;
        struct {
            Pyo** items;
            long long len, cap;
        } list;
        struct {
            Pyo** items;
            long long len;
        } tuple;
        struct {
            Pyo** keys;
            Pyo** vals;
            long long len, cap;
        } dict;
        struct {
            Pyo** items;
            long long len, cap;
        } set;
        struct {
            long long start, stop, step;
        } range;
        struct {
            PyFn fn;
            const char* name;
            const char** params;
            int nparams;
            Pyo** defaults;
            int ndefaults;
            int vararg;
            int kwarg;
            int is_builtin;
        } func;
        struct {
            Pyo* src;
            long long idx;
        } iter;
    } u;
};

static long long range_len(Pyo* r);

void py_runtime_error(const char* fmt, ...) {    va_list ap;
    fputs("Traceback (most recent call last):\n", stderr);
    fputs("RuntimeError: ", stderr);
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
    fputc('\n', stderr);
    exit(1);
}

static Pyo* py_new(int kind) {
    Pyo* o = (Pyo*)malloc(sizeof(Pyo));
    if (!o) py_runtime_error("out of memory");
    memset(o, 0, sizeof(Pyo));
    o->kind = kind;
    return o;
}

Pyo* py_none(void) {
    static Pyo none = {.kind = PY_NONE};
    return &none;
}
Pyo* py_true(void) {
    static Pyo t = {.kind = PY_BOOL, .u = {.b = 1}};
    return &t;
}
Pyo* py_false(void) {
    static Pyo f = {.kind = PY_BOOL, .u = {.b = 0}};
    return &f;
}
Pyo* py_bool(int b) { return b ? py_true() : py_false(); }
Pyo* py_int(long long v) {
    Pyo* o = py_new(PY_INT);
    o->u.i = v;
    return o;
}
Pyo* py_float(double v) {
    Pyo* o = py_new(PY_FLOAT);
    o->u.f = v;
    return o;
}
Pyo* py_strn(const char* s, long long len) {
    Pyo* o = py_new(PY_STR);
    o->u.str.s = (char*)malloc((size_t)len + 1);
    if (!o->u.str.s) py_runtime_error("out of memory");
    if (len) memcpy(o->u.str.s, s, (size_t)len);
    o->u.str.s[len] = '\0';
    o->u.str.len = len;
    return o;
}
Pyo* py_str(const char* s) { return py_strn(s, (long long)strlen(s)); }

static void list_reserve(Pyo* o, long long extra) {
    if (o->u.list.len + extra <= o->u.list.cap) return;
    long long nc = o->u.list.cap ? o->u.list.cap * 2 : 8;
    while (nc < o->u.list.len + extra) nc *= 2;
    o->u.list.items = (Pyo**)realloc(o->u.list.items, (size_t)nc * sizeof(Pyo*));
    if (!o->u.list.items) py_runtime_error("out of memory");
    o->u.list.cap = nc;
}
static void list_push(Pyo* o, Pyo* v) {
    list_reserve(o, 1);
    o->u.list.items[o->u.list.len++] = v;
}

Pyo* py_list(Pyo** items, int n) {
    Pyo* o = py_new(PY_LIST);
    for (int i = 0; i < n; i++) list_push(o, items[i]);
    return o;
}
Pyo* py_tuple(Pyo** items, int n) {
    Pyo* o = py_new(PY_TUPLE);
    o->u.tuple.len = n;
    o->u.tuple.items = n ? (Pyo**)malloc((size_t)n * sizeof(Pyo*)) : NULL;
    if (n && !o->u.tuple.items) py_runtime_error("out of memory");
    for (int i = 0; i < n; i++) o->u.tuple.items[i] = items[i];
    return o;
}
static void dict_reserve(Pyo* o, long long extra) {
    if (o->u.dict.len + extra <= o->u.dict.cap) return;
    long long nc = o->u.dict.cap ? o->u.dict.cap * 2 : 8;
    while (nc < o->u.dict.len + extra) nc *= 2;
    o->u.dict.keys = (Pyo**)realloc(o->u.dict.keys, (size_t)nc * sizeof(Pyo*));
    o->u.dict.vals = (Pyo**)realloc(o->u.dict.vals, (size_t)nc * sizeof(Pyo*));
    if (!o->u.dict.keys || !o->u.dict.vals) py_runtime_error("out of memory");
    o->u.dict.cap = nc;
}
Pyo* py_dict(Pyo** keys, Pyo** vals, int n) {
    Pyo* o = py_new(PY_DICT);
    for (int i = 0; i < n; i++) {
        dict_reserve(o, 1);
        o->u.dict.keys[o->u.dict.len] = keys[i];
        o->u.dict.vals[o->u.dict.len] = vals[i];
        o->u.dict.len++;
    }
    return o;
}
static void set_reserve(Pyo* o, long long extra) {
    if (o->u.set.len + extra <= o->u.set.cap) return;
    long long nc = o->u.set.cap ? o->u.set.cap * 2 : 8;
    while (nc < o->u.set.len + extra) nc *= 2;
    o->u.set.items = (Pyo**)realloc(o->u.set.items, (size_t)nc * sizeof(Pyo*));
    if (!o->u.set.items) py_runtime_error("out of memory");
    o->u.set.cap = nc;
}
static void set_add(Pyo* o, Pyo* v);
Pyo* py_set(Pyo** items, int n) {
    Pyo* o = py_new(PY_SET);
    for (int i = 0; i < n; i++) set_add(o, items[i]);
    return o;
}
Pyo* py_slice(Pyo* lower, Pyo* upper, Pyo* step) {
    Pyo* o = py_new(PY_ITER + 100); /* unique kind for slices */
    o->u.list.items = (Pyo**)malloc(3 * sizeof(Pyo*));
    if (!o->u.list.items) py_runtime_error("out of memory");
    o->u.list.items[0] = lower;
    o->u.list.items[1] = upper;
    o->u.list.items[2] = step;
    o->u.list.len = 3;
    return o;
}
#define PY_SLICE (PY_ITER + 100)

Pyo* py_func(PyFn fn, const char* name, const char** params, int nparams,
             Pyo** defaults, int ndefaults, int vararg, int kwarg) {
    Pyo* o = py_new(PY_FUNC);
    o->u.func.fn = fn;
    o->u.func.name = name;
    o->u.func.params = params;
    o->u.func.nparams = nparams;
    o->u.func.defaults = defaults;
    o->u.func.ndefaults = ndefaults;
    o->u.func.vararg = vararg;
    o->u.func.kwarg = kwarg;
    o->u.func.is_builtin = 0;
    return o;
}

Pyo* py_builtin_ref(const char* name) {
    Pyo* o = py_new(PY_FUNC);
    o->u.func.fn = NULL;
    o->u.func.name = name;
    o->u.func.params = NULL;
    o->u.func.nparams = 0;
    o->u.func.defaults = NULL;
    o->u.func.ndefaults = 0;
    o->u.func.vararg = 1;
    o->u.func.kwarg = 0;
    o->u.func.is_builtin = 1;
    return o;
}

static Pyo* py_call_builtin(const char* name, Pyo** args, int argc, Pyo** kwnames, Pyo** kwvals,
                            int nkw);

/* ---------------- string builder ---------------- */
typedef struct {
    char* data;
    size_t len, cap;
} SB;
static void sb_init(SB* b) {
    b->data = NULL;
    b->len = 0;
    b->cap = 0;
}
static void sb_reserve(SB* b, size_t extra) {
    if (b->len + extra + 1 <= b->cap) return;
    size_t nc = b->cap ? b->cap * 2 : 32;
    while (nc < b->len + extra + 1) nc *= 2;
    b->data = (char*)realloc(b->data, nc);
    if (!b->data) py_runtime_error("out of memory");
    b->cap = nc;
}
static void sb_putc(SB* b, char c) {
    sb_reserve(b, 1);
    b->data[b->len++] = c;
    b->data[b->len] = '\0';
}
static void sb_putn(SB* b, const char* s, size_t n) {
    sb_reserve(b, n);
    memcpy(b->data + b->len, s, n);
    b->len += n;
    b->data[b->len] = '\0';
}
static void sb_puts(SB* b, const char* s) { sb_putn(b, s, strlen(s)); }

static void fmt_double(char* buf, size_t n, double d) {
    if (isnan(d)) {
        snprintf(buf, n, "nan");
        return;
    }
    if (isinf(d)) {
        snprintf(buf, n, d < 0 ? "-inf" : "inf");
        return;
    }
    for (int prec = 15; prec <= 17; prec++) {
        snprintf(buf, n, "%.*g", prec, d);
        if (strtod(buf, NULL) == d) break;
    }
    if (!strpbrk(buf, ".eEnN")) strncat(buf, ".0", n - strlen(buf) - 1);
}

static void repr_str(SB* b, const char* s, long long len) {
    int has_single = 0, has_double = 0;
    for (long long i = 0; i < len; i++) {
        if (s[i] == '\'') has_single = 1;
        if (s[i] == '"') has_double = 1;
    }
    char q = (has_single && !has_double) ? '"' : '\'';
    sb_putc(b, q);
    for (long long i = 0; i < len; i++) {
        unsigned char c = (unsigned char)s[i];
        if (c == (unsigned char)q || c == '\\') {
            sb_putc(b, '\\');
            sb_putc(b, (char)c);
        } else if (c == '\n') {
            sb_puts(b, "\\n");
        } else if (c == '\t') {
            sb_puts(b, "\\t");
        } else if (c == '\r') {
            sb_puts(b, "\\r");
        } else if (c < 32) {
            char tmp[8];
            snprintf(tmp, sizeof(tmp), "\\x%02x", c);
            sb_puts(b, tmp);
        } else {
            sb_putc(b, (char)c);
        }
    }
    sb_putc(b, q);
}

static void to_buf(SB* b, Pyo* v, int repr);

static void to_buf(SB* b, Pyo* v, int repr) {
    char tmp[64];
    switch (v->kind) {
        case PY_NONE:
            sb_puts(b, "None");
            break;
        case PY_BOOL:
            sb_puts(b, v->u.b ? "True" : "False");
            break;
        case PY_INT:
            snprintf(tmp, sizeof(tmp), "%lld", v->u.i);
            sb_puts(b, tmp);
            break;
        case PY_FLOAT:
            fmt_double(tmp, sizeof(tmp), v->u.f);
            sb_puts(b, tmp);
            break;
        case PY_STR:
            if (repr)
                repr_str(b, v->u.str.s, v->u.str.len);
            else
                sb_putn(b, v->u.str.s, (size_t)v->u.str.len);
            break;
        case PY_LIST:
            sb_putc(b, '[');
            for (long long i = 0; i < v->u.list.len; i++) {
                if (i) sb_puts(b, ", ");
                to_buf(b, v->u.list.items[i], 1);
            }
            sb_putc(b, ']');
            break;
        case PY_TUPLE:
            sb_putc(b, '(');
            for (long long i = 0; i < v->u.tuple.len; i++) {
                if (i) sb_puts(b, ", ");
                to_buf(b, v->u.tuple.items[i], 1);
            }
            if (v->u.tuple.len == 1) sb_putc(b, ',');
            sb_putc(b, ')');
            break;
        case PY_DICT:
            sb_putc(b, '{');
            for (long long i = 0; i < v->u.dict.len; i++) {
                if (i) sb_puts(b, ", ");
                to_buf(b, v->u.dict.keys[i], 1);
                sb_puts(b, ": ");
                to_buf(b, v->u.dict.vals[i], 1);
            }
            sb_putc(b, '}');
            break;
        case PY_SET:
            if (v->u.set.len == 0) {
                sb_puts(b, "set()");
            } else {
                sb_putc(b, '{');
                for (long long i = 0; i < v->u.set.len; i++) {
                    if (i) sb_puts(b, ", ");
                    to_buf(b, v->u.set.items[i], 1);
                }
                sb_putc(b, '}');
            }
            break;
        case PY_RANGE:
            snprintf(tmp, sizeof(tmp), "range(%lld, %lld, %lld)", v->u.range.start,
                     v->u.range.stop, v->u.range.step);
            sb_puts(b, tmp);
            break;
        case PY_FUNC:
            snprintf(tmp, sizeof(tmp), "<function %s>", v->u.func.name);
            sb_puts(b, tmp);
            break;
        case PY_ITER:
            sb_puts(b, "<iterator>");
            break;
        default:
            sb_puts(b, "<object>");
            break;
    }
}

static Pyo* to_string(Pyo* v, int repr) {
    SB b;
    sb_init(&b);
    to_buf(&b, v, repr);
    if (!b.data) return py_str("");
    Pyo* o = py_strn(b.data, (long long)b.len);
    free(b.data);
    return o;
}

Pyo* py_builtin_str(Pyo* x) { return to_string(x, 0); }
Pyo* py_builtin_repr(Pyo* x) { return to_string(x, 1); }
void py_assert(int cond, Pyo* msg) {
    if (cond) return;
    if (msg && msg->kind != PY_NONE) {
        Pyo* s = to_string(msg, 0);
        py_runtime_error("AssertionError: %.*s", (int)s->u.str.len, s->u.str.s);
    }
    py_runtime_error("AssertionError");
}

/* ---------------- numeric helpers ---------------- */
static int is_num(Pyo* v) { return v->kind == PY_INT || v->kind == PY_FLOAT || v->kind == PY_BOOL; }
static long long as_int(Pyo* v) {
    if (v->kind == PY_INT) return v->u.i;
    if (v->kind == PY_BOOL) return v->u.b;
    if (v->kind == PY_FLOAT) return (long long)v->u.f;
    py_runtime_error("expected an integer, got %s", to_string(py_type(v), 0)->u.str.s);
    return 0;
}
static double as_double(Pyo* v) {
    if (v->kind == PY_INT) return (double)v->u.i;
    if (v->kind == PY_BOOL) return (double)v->u.b;
    if (v->kind == PY_FLOAT) return v->u.f;
    py_runtime_error("expected a number");
    return 0;
}
static int both_int(Pyo* a, Pyo* b) {
    return (a->kind == PY_INT || a->kind == PY_BOOL) && (b->kind == PY_INT || b->kind == PY_BOOL);
}
static const char* kind_name(int k) {
    switch (k) {
        case PY_NONE: return "NoneType";
        case PY_BOOL: return "bool";
        case PY_INT: return "int";
        case PY_FLOAT: return "float";
        case PY_STR: return "str";
        case PY_LIST: return "list";
        case PY_TUPLE: return "tuple";
        case PY_DICT: return "dict";
        case PY_SET: return "set";
        case PY_RANGE: return "range";
        case PY_FUNC: return "function";
        case PY_ITER: return "iterator";
        default: return "slice";
    }
}
Pyo* py_type(Pyo* x) { return py_str(kind_name(x->kind)); }

/* ---------------- operators ---------------- */
static Pyo* repeat(Pyo* a, long long n) {
    if (a->kind == PY_STR) {
        if (n < 0) n = 0;
        SB b;
        sb_init(&b);
        for (long long i = 0; i < n; i++) sb_putn(&b, a->u.str.s, (size_t)a->u.str.len);
        Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
        free(b.data);
        return o;
    }
    if (a->kind == PY_LIST || a->kind == PY_TUPLE) {
        if (n < 0) n = 0;
        long long len = a->kind == PY_LIST ? a->u.list.len : a->u.tuple.len;
        Pyo** src = a->kind == PY_LIST ? a->u.list.items : a->u.tuple.items;
        Pyo** items = (Pyo**)malloc((size_t)(len * n + 1) * sizeof(Pyo*));
        for (long long i = 0; i < n; i++)
            for (long long j = 0; j < len; j++) items[i * len + j] = src[j];
        Pyo* o = a->kind == PY_LIST ? py_list(items, (int)(len * n)) : py_tuple(items, (int)(len * n));
        free(items);
        return o;
    }
    py_runtime_error("can't multiply sequence by non-int");
    return py_none();
}

Pyo* py_add(Pyo* a, Pyo* b) {
    if (both_int(a, b)) return py_int(as_int(a) + as_int(b));
    if (is_num(a) && is_num(b)) return py_float(as_double(a) + as_double(b));
    if (a->kind == PY_STR && b->kind == PY_STR) {
        SB s;
        sb_init(&s);
        sb_putn(&s, a->u.str.s, (size_t)a->u.str.len);
        sb_putn(&s, b->u.str.s, (size_t)b->u.str.len);
        Pyo* o = py_strn(s.data ? s.data : "", s.data ? (long long)s.len : 0);
        free(s.data);
        return o;
    }
    if (a->kind == PY_LIST && b->kind == PY_LIST) {
        Pyo* o = py_list(a->u.list.items, (int)a->u.list.len);
        for (long long i = 0; i < b->u.list.len; i++) list_push(o, b->u.list.items[i]);
        return o;
    }
    if (a->kind == PY_TUPLE && b->kind == PY_TUPLE) {
        long long n = a->u.tuple.len + b->u.tuple.len;
        Pyo** items = (Pyo**)malloc((size_t)(n + 1) * sizeof(Pyo*));
        for (long long i = 0; i < a->u.tuple.len; i++) items[i] = a->u.tuple.items[i];
        for (long long i = 0; i < b->u.tuple.len; i++) items[a->u.tuple.len + i] = b->u.tuple.items[i];
        Pyo* o = py_tuple(items, (int)n);
        free(items);
        return o;
    }
    py_runtime_error("unsupported operand type(s) for +: '%s' and '%s'", kind_name(a->kind),
                     kind_name(b->kind));
    return py_none();
}

Pyo* py_sub(Pyo* a, Pyo* b) {
    if (both_int(a, b)) return py_int(as_int(a) - as_int(b));
    if (is_num(a) && is_num(b)) return py_float(as_double(a) - as_double(b));
    if (a->kind == PY_SET && b->kind == PY_SET) {
        Pyo* o = py_new(PY_SET);
        for (long long i = 0; i < a->u.set.len; i++)
            if (py_truth(py_in(a->u.set.items[i], b)) == 0) set_add(o, a->u.set.items[i]);
        return o;
    }
    py_runtime_error("unsupported operand type(s) for -: '%s' and '%s'", kind_name(a->kind),
                     kind_name(b->kind));
    return py_none();
}

Pyo* py_mul(Pyo* a, Pyo* b) {
    if (both_int(a, b)) return py_int(as_int(a) * as_int(b));
    if (is_num(a) && is_num(b)) return py_float(as_double(a) * as_double(b));
    if ((a->kind == PY_STR || a->kind == PY_LIST || a->kind == PY_TUPLE) && b->kind == PY_INT)
        return repeat(a, b->u.i);
    if ((b->kind == PY_STR || b->kind == PY_LIST || b->kind == PY_TUPLE) && a->kind == PY_INT)
        return repeat(b, a->u.i);
    py_runtime_error("unsupported operand type(s) for *: '%s' and '%s'", kind_name(a->kind),
                     kind_name(b->kind));
    return py_none();
}

Pyo* py_div(Pyo* a, Pyo* b) {
    if (!is_num(a) || !is_num(b)) py_runtime_error("unsupported operand type(s) for /");
    double d = as_double(b);
    if (d == 0.0) py_runtime_error("division by zero");
    return py_float(as_double(a) / d);
}

Pyo* py_floordiv(Pyo* a, Pyo* b) {
    if (both_int(a, b)) {
        long long x = as_int(a), y = as_int(b);
        if (y == 0) py_runtime_error("integer division or modulo by zero");
        long long q = x / y;
        if ((x % y != 0) && ((x < 0) != (y < 0))) q--;
        return py_int(q);
    }
    if (!is_num(a) || !is_num(b)) py_runtime_error("unsupported operand type(s) for //");
    double y = as_double(b);
    if (y == 0.0) py_runtime_error("float floor division by zero");
    return py_float(floor(as_double(a) / y));
}

Pyo* py_mod(Pyo* a, Pyo* b) {
    if (both_int(a, b)) {
        long long x = as_int(a), y = as_int(b);
        if (y == 0) py_runtime_error("integer division or modulo by zero");
        long long r = x % y;
        if (r != 0 && ((r < 0) != (y < 0))) r += y;
        return py_int(r);
    }
    if (is_num(a) && is_num(b)) {
        double y = as_double(b);
        if (y == 0.0) py_runtime_error("float modulo by zero");
        double r = fmod(as_double(a), y);
        if (r != 0.0 && ((r < 0) != (y < 0))) r += y;
        return py_float(r);
    }
    if (a->kind == PY_STR) {
        py_runtime_error("%% string formatting is not supported; use f-strings");
    }
    py_runtime_error("unsupported operand type(s) for %%");
    return py_none();
}

Pyo* py_pow(Pyo* a, Pyo* b) {
    if (both_int(a, b) && as_int(b) >= 0) {
        long long base = as_int(a), e = as_int(b), r = 1;
        while (e > 0) {
            if (e & 1) r *= base;
            base *= base;
            e >>= 1;
        }
        return py_int(r);
    }
    if (is_num(a) && is_num(b)) return py_float(pow(as_double(a), as_double(b)));
    py_runtime_error("unsupported operand type(s) for **");
    return py_none();
}
Pyo* py_pow_builtin(Pyo* a, Pyo* b) { return py_pow(a, b); }

Pyo* py_lshift(Pyo* a, Pyo* b) {
    if (!both_int(a, b)) py_runtime_error("unsupported operand type(s) for <<");
    return py_int(as_int(a) << as_int(b));
}
Pyo* py_rshift(Pyo* a, Pyo* b) {
    if (!both_int(a, b)) py_runtime_error("unsupported operand type(s) for >>");
    return py_int(as_int(a) >> as_int(b));
}
Pyo* py_bitand(Pyo* a, Pyo* b) {
    if (a->kind == PY_SET && b->kind == PY_SET) {
        Pyo* o = py_new(PY_SET);
        for (long long i = 0; i < a->u.set.len; i++)
            if (py_truth(py_in(a->u.set.items[i], b))) set_add(o, a->u.set.items[i]);
        return o;
    }
    if (!both_int(a, b)) py_runtime_error("unsupported operand type(s) for &");
    return py_int(as_int(a) & as_int(b));
}
Pyo* py_bitor(Pyo* a, Pyo* b) {
    if (a->kind == PY_SET && b->kind == PY_SET) {
        Pyo* o = py_list(a->u.set.items, (int)a->u.set.len);
        Pyo* copy = py_set(o->u.list.items, (int)o->u.list.len);
        for (long long i = 0; i < b->u.set.len; i++) set_add(copy, b->u.set.items[i]);
        return copy;
    }
    if (!both_int(a, b)) py_runtime_error("unsupported operand type(s) for |");
    return py_int(as_int(a) | as_int(b));
}
Pyo* py_bitxor(Pyo* a, Pyo* b) {
    if (!both_int(a, b)) py_runtime_error("unsupported operand type(s) for ^");
    return py_int(as_int(a) ^ as_int(b));
}
Pyo* py_neg(Pyo* a) {
    if (a->kind == PY_INT) return py_int(-a->u.i);
    if (a->kind == PY_BOOL) return py_int(-a->u.b);
    if (a->kind == PY_FLOAT) return py_float(-a->u.f);
    py_runtime_error("bad operand type for unary -: '%s'", kind_name(a->kind));
    return py_none();
}
Pyo* py_pos(Pyo* a) {
    if (is_num(a)) return a;
    py_runtime_error("bad operand type for unary +");
    return py_none();
}
Pyo* py_invert(Pyo* a) {
    if (!both_int(a, a)) py_runtime_error("bad operand type for unary ~");
    return py_int(~as_int(a));
}

int py_truth(Pyo* a) {
    switch (a->kind) {
        case PY_NONE: return 0;
        case PY_BOOL: return a->u.b;
        case PY_INT: return a->u.i != 0;
        case PY_FLOAT: return a->u.f != 0.0;
        case PY_STR: return a->u.str.len > 0;
        case PY_LIST: return a->u.list.len > 0;
        case PY_TUPLE: return a->u.tuple.len > 0;
        case PY_DICT: return a->u.dict.len > 0;
        case PY_SET: return a->u.set.len > 0;
        case PY_RANGE: {
            long long n = range_len(a);
            return n > 0;
        }
        default: return 1;
    }
}
Pyo* py_not(Pyo* a) { return py_bool(!py_truth(a)); }

static long long range_len(Pyo* r) {
    long long start = r->u.range.start, stop = r->u.range.stop, step = r->u.range.step;
    if (step > 0) {
        if (stop <= start) return 0;
        return (stop - start + step - 1) / step;
    } else if (step < 0) {
        if (stop >= start) return 0;
        return (start - stop + (-step) - 1) / (-step);
    }
    return 0;
}

/* ---------------- equality and ordering ---------------- */
static int seq_equal(long long n, Pyo** ai, Pyo** bi);

static int dict_equal(Pyo* a, Pyo* b) {
    if (a->u.dict.len != b->u.dict.len) return 0;
    for (long long i = 0; i < a->u.dict.len; i++) {
        int found = 0;
        for (long long j = 0; j < b->u.dict.len; j++) {
            if (py_truth(py_eq(a->u.dict.keys[i], b->u.dict.keys[j]))) {
                if (!py_truth(py_eq(a->u.dict.vals[i], b->u.dict.vals[j]))) return 0;
                found = 1;
                break;
            }
        }
        if (!found) return 0;
    }
    return 1;
}

static int py_equal(Pyo* a, Pyo* b) {
    if (a == b) return 1;
    if (is_num(a) && is_num(b)) return as_double(a) == as_double(b);
    if (a->kind != b->kind) return 0;
    switch (a->kind) {
        case PY_NONE: return 1;
        case PY_STR:
            return a->u.str.len == b->u.str.len &&
                   memcmp(a->u.str.s, b->u.str.s, (size_t)a->u.str.len) == 0;
        case PY_LIST:
            return seq_equal(a->u.list.len, a->u.list.items, b->u.list.items);
        case PY_TUPLE:
            return seq_equal(a->u.tuple.len, a->u.tuple.items, b->u.tuple.items);
        case PY_DICT: return dict_equal(a, b);
        case PY_SET: {
            if (a->u.set.len != b->u.set.len) return 0;
            for (long long i = 0; i < a->u.set.len; i++)
                if (!py_truth(py_in(a->u.set.items[i], b))) return 0;
            return 1;
        }
        case PY_RANGE:
            return a->u.range.start == b->u.range.start && a->u.range.stop == b->u.range.stop &&
                   a->u.range.step == b->u.range.step;
        default: return 0;
    }
}
static int seq_equal(long long n, Pyo** ai, Pyo** bi) {
    for (long long i = 0; i < n; i++)
        if (!py_equal(ai[i], bi[i])) return 0;
    return 1;
}

Pyo* py_eq(Pyo* a, Pyo* b) { return py_bool(py_equal(a, b)); }
Pyo* py_ne(Pyo* a, Pyo* b) { return py_bool(!py_equal(a, b)); }

static int py_less(Pyo* a, Pyo* b) {
    if (is_num(a) && is_num(b)) return as_double(a) < as_double(b);
    if (a->kind == PY_STR && b->kind == PY_STR) {
        long long n = a->u.str.len < b->u.str.len ? a->u.str.len : b->u.str.len;
        int c = memcmp(a->u.str.s, b->u.str.s, (size_t)n);
        if (c != 0) return c < 0;
        return a->u.str.len < b->u.str.len;
    }
    if ((a->kind == PY_LIST && b->kind == PY_LIST) || (a->kind == PY_TUPLE && b->kind == PY_TUPLE)) {
        long long n = a->kind == PY_LIST ? a->u.list.len : a->u.tuple.len;
        long long m = b->kind == PY_LIST ? b->u.list.len : b->u.tuple.len;
        Pyo** ai = a->kind == PY_LIST ? a->u.list.items : a->u.tuple.items;
        Pyo** bi = b->kind == PY_LIST ? b->u.list.items : b->u.tuple.items;
        for (long long i = 0; i < n && i < m; i++) {
            if (py_equal(ai[i], bi[i])) continue;
            return py_less(ai[i], bi[i]);
        }
        return n < m;
    }
    py_runtime_error("'<' not supported between instances of '%s' and '%s'", kind_name(a->kind),
                     kind_name(b->kind));
    return 0;
}
Pyo* py_lt(Pyo* a, Pyo* b) { return py_bool(py_less(a, b)); }
Pyo* py_le(Pyo* a, Pyo* b) { return py_bool(py_less(a, b) || py_equal(a, b)); }
Pyo* py_gt(Pyo* a, Pyo* b) { return py_bool(py_less(b, a)); }
Pyo* py_ge(Pyo* a, Pyo* b) { return py_bool(py_less(b, a) || py_equal(a, b)); }
Pyo* py_is(Pyo* a, Pyo* b) {
    if (a == b) return py_true();
    if (a->kind == PY_NONE && b->kind == PY_NONE) return py_true();
    if (a->kind == PY_BOOL && b->kind == PY_BOOL) return py_bool(a->u.b == b->u.b);
    return py_false();
}
Pyo* py_isnot(Pyo* a, Pyo* b) { return py_bool(!py_truth(py_is(a, b))); }

int py_contains_impl(Pyo* container, Pyo* item);
int py_contains_impl(Pyo* container, Pyo* item) {
    switch (container->kind) {
        case PY_STR:
            if (item->kind != PY_STR) py_runtime_error("'in <string>' requires string as left operand");
            if (item->u.str.len == 0) return 1;
            for (long long i = 0; i + item->u.str.len <= container->u.str.len; i++)
                if (memcmp(container->u.str.s + i, item->u.str.s, (size_t)item->u.str.len) == 0)
                    return 1;
            return 0;
        case PY_LIST:
            for (long long i = 0; i < container->u.list.len; i++)
                if (py_equal(container->u.list.items[i], item)) return 1;
            return 0;
        case PY_TUPLE:
            for (long long i = 0; i < container->u.tuple.len; i++)
                if (py_equal(container->u.tuple.items[i], item)) return 1;
            return 0;
        case PY_DICT:
            for (long long i = 0; i < container->u.dict.len; i++)
                if (py_equal(container->u.dict.keys[i], item)) return 1;
            return 0;
        case PY_SET:
            for (long long i = 0; i < container->u.set.len; i++)
                if (py_equal(container->u.set.items[i], item)) return 1;
            return 0;
        case PY_RANGE: {
            if (!both_int(item, item)) return 0;
            long long v = as_int(item), s = container->u.range.start;
            long long e = container->u.range.stop, st = container->u.range.step;
            if (st > 0) return v >= s && v < e && (v - s) % st == 0;
            if (st < 0) return v <= s && v > e && (s - v) % (-st) == 0;
            return 0;
        }
        default:
            py_runtime_error("argument of type '%s' is not iterable", kind_name(container->kind));
            return 0;
    }
}
Pyo* py_in(Pyo* item, Pyo* container) { return py_bool(py_contains_impl(container, item)); }

/* ---------------- sequences ---------------- */
Pyo* py_len(Pyo* o) {
    switch (o->kind) {
        case PY_STR: return py_int(o->u.str.len);
        case PY_LIST: return py_int(o->u.list.len);
        case PY_TUPLE: return py_int(o->u.tuple.len);
        case PY_DICT: return py_int(o->u.dict.len);
        case PY_SET: return py_int(o->u.set.len);
        case PY_RANGE: return py_int(range_len(o));
        default:
            py_runtime_error("object of type '%s' has no len()", kind_name(o->kind));
            return py_none();
    }
}

static long long normalize_index(long long i, long long n) {
    if (i < 0) i += n;
    if (i < 0 || i >= n) py_runtime_error("index out of range");
    return i;
}

static Pyo* slice_apply(Pyo* obj, Pyo* sl);

Pyo* py_getitem(Pyo* obj, Pyo* key) {
    if (key->kind == PY_SLICE) return slice_apply(obj, key);
    switch (obj->kind) {
        case PY_LIST:
            return obj->u.list.items[normalize_index(as_int(key), obj->u.list.len)];
        case PY_TUPLE:
            return obj->u.tuple.items[normalize_index(as_int(key), obj->u.tuple.len)];
        case PY_STR: {
            long long i = normalize_index(as_int(key), obj->u.str.len);
            return py_strn(obj->u.str.s + i, 1);
        }
        case PY_DICT:
            for (long long i = 0; i < obj->u.dict.len; i++)
                if (py_equal(obj->u.dict.keys[i], key)) return obj->u.dict.vals[i];
            py_runtime_error("KeyError: %s", to_string(key, 1)->u.str.s);
            return py_none();
        case PY_RANGE: {
            long long i = normalize_index(as_int(key), range_len(obj));
            return py_int(obj->u.range.start + i * obj->u.range.step);
        }
        default:
            py_runtime_error("'%s' object is not subscriptable", kind_name(obj->kind));
            return py_none();
    }
}

void py_setitem(Pyo* obj, Pyo* key, Pyo* value) {
    if (obj->kind == PY_LIST) {
        obj->u.list.items[normalize_index(as_int(key), obj->u.list.len)] = value;
        return;
    }
    if (obj->kind == PY_DICT) {
        for (long long i = 0; i < obj->u.dict.len; i++) {
            if (py_equal(obj->u.dict.keys[i], key)) {
                obj->u.dict.vals[i] = value;
                return;
            }
        }
        dict_reserve(obj, 1);
        obj->u.dict.keys[obj->u.dict.len] = key;
        obj->u.dict.vals[obj->u.dict.len] = value;
        obj->u.dict.len++;
        return;
    }
    py_runtime_error("'%s' object does not support item assignment", kind_name(obj->kind));
}

static long long clamp_slice(long long v, long long n, long long step) {
    if (v < 0) v += n;
    if (step > 0) {
        if (v < 0) v = 0;
        if (v > n) v = n;
    } else {
        if (v < -1) v = -1;
        if (v > n) v = n;
    }
    return v;
}

static Pyo* slice_apply(Pyo* obj, Pyo* sl) {
    long long n = 0;
    if (obj->kind == PY_LIST) n = obj->u.list.len;
    else if (obj->kind == PY_TUPLE) n = obj->u.tuple.len;
    else if (obj->kind == PY_STR) n = obj->u.str.len;
    else py_runtime_error("'%s' object is not sliceable", kind_name(obj->kind));
    Pyo* lo = sl->u.list.items[0];
    Pyo* up = sl->u.list.items[1];
    Pyo* st = sl->u.list.items[2];
    long long step = st && st->kind != PY_NONE ? as_int(st) : 1;
    if (step == 0) py_runtime_error("slice step cannot be zero");
    long long start, stop;
    if (step > 0) {
        start = (lo && lo->kind != PY_NONE) ? clamp_slice(as_int(lo), n, 1) : 0;
        stop = (up && up->kind != PY_NONE) ? clamp_slice(as_int(up), n, 1) : n;
    } else {
        start = (lo && lo->kind != PY_NONE) ? clamp_slice(as_int(lo), n, -1) : n - 1;
        stop = (up && up->kind != PY_NONE) ? clamp_slice(as_int(up), n, -1) : -1;
    }
    if (obj->kind == PY_STR) {
        SB b;
        sb_init(&b);
        if (step > 0)
            for (long long i = start; i < stop; i += step) sb_putc(&b, obj->u.str.s[i]);
        else
            for (long long i = start; i > stop; i += step) sb_putc(&b, obj->u.str.s[i]);
        Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
        free(b.data);
        return o;
    }
    Pyo** items = (Pyo**)malloc((size_t)(n + 1) * sizeof(Pyo*));
    long long cnt = 0;
    if (step > 0)
        for (long long i = start; i < stop; i += step) items[cnt++] = obj->kind == PY_LIST ? obj->u.list.items[i] : obj->u.tuple.items[i];
    else
        for (long long i = start; i > stop; i += step) items[cnt++] = obj->kind == PY_LIST ? obj->u.list.items[i] : obj->u.tuple.items[i];
    Pyo* o = obj->kind == PY_LIST ? py_list(items, (int)cnt) : py_tuple(items, (int)cnt);
    free(items);
    return o;
}

/* ---------------- iteration ---------------- */
Pyo* py_iter(Pyo* obj) {
    if (obj->kind == PY_ITER) return obj;
    if (obj->kind == PY_RANGE || obj->kind == PY_LIST || obj->kind == PY_TUPLE ||
        obj->kind == PY_STR || obj->kind == PY_DICT || obj->kind == PY_SET) {
        Pyo* it = py_new(PY_ITER);
        it->u.iter.src = obj;
        it->u.iter.idx = 0;
        return it;
    }
    py_runtime_error("'%s' object is not iterable", kind_name(obj->kind));
    return py_none();
}

int py_iter_next(Pyo* it, Pyo** out) {
    if (it->kind != PY_ITER) py_runtime_error("expected an iterator");
    Pyo* src = it->u.iter.src;
    long long i = it->u.iter.idx;
    switch (src->kind) {
        case PY_RANGE: {
            long long n = range_len(src);
            if (i >= n) return 0;
            *out = py_int(src->u.range.start + i * src->u.range.step);
            it->u.iter.idx = i + 1;
            return 1;
        }
        case PY_LIST:
            if (i >= src->u.list.len) return 0;
            *out = src->u.list.items[i];
            it->u.iter.idx = i + 1;
            return 1;
        case PY_TUPLE:
            if (i >= src->u.tuple.len) return 0;
            *out = src->u.tuple.items[i];
            it->u.iter.idx = i + 1;
            return 1;
        case PY_STR:
            if (i >= src->u.str.len) return 0;
            *out = py_strn(src->u.str.s + i, 1);
            it->u.iter.idx = i + 1;
            return 1;
        case PY_DICT:
            if (i >= src->u.dict.len) return 0;
            *out = src->u.dict.keys[i];
            it->u.iter.idx = i + 1;
            return 1;
        case PY_SET:
            if (i >= src->u.set.len) return 0;
            *out = src->u.set.items[i];
            it->u.iter.idx = i + 1;
            return 1;
        default:
            return 0;
    }
}

/* ---------------- calls ---------------- */
Pyo* py_call_kw(Pyo* fn, Pyo** args, int argc, Pyo** kwnames, Pyo** kwvals, int nkw) {
    if (fn->kind != PY_FUNC) py_runtime_error("'%s' object is not callable", kind_name(fn->kind));
    if (fn->u.func.is_builtin)
        return py_call_builtin(fn->u.func.name, args, argc, kwnames, kwvals, nkw);
    int np = fn->u.func.nparams;
    int has_v = fn->u.func.vararg;
    int has_k = fn->u.func.kwarg;
    int nslots = np + has_v + has_k;
    Pyo** bound = (Pyo**)malloc((size_t)(nslots + 1) * sizeof(Pyo*));
    int* filled = (int*)calloc((size_t)(np + 1), sizeof(int));
    if (argc > np && !has_v) {
        py_runtime_error("%s() takes %d positional argument%s but %d were given", fn->u.func.name,
                         np, np == 1 ? "" : "s", argc);
    }
    for (int i = 0; i < argc && i < np; i++) {
        bound[i] = args[i];
        filled[i] = 1;
    }
    if (has_v) {
        long long extra = argc > np ? argc - np : 0;
        Pyo** rest = extra ? (Pyo**)malloc((size_t)extra * sizeof(Pyo*)) : NULL;
        for (long long i = 0; i < extra; i++) rest[i] = args[np + i];
        bound[np] = py_tuple(rest, (int)extra);
        free(rest);
    }
    Pyo* kdict = has_k ? py_new(PY_DICT) : NULL;
    for (int k = 0; k < nkw; k++) {
        if (kwnames[k]->kind != PY_STR) py_runtime_error("keywords must be strings");
        const char* name = kwnames[k]->u.str.s;
        int found = -1;
        for (int p = 0; p < np; p++)
            if (strcmp(fn->u.func.params[p], name) == 0) {
                found = p;
                break;
            }
        if (found >= 0) {
            if (filled[found])
                py_runtime_error("%s() got multiple values for argument '%s'", fn->u.func.name, name);
            bound[found] = kwvals[k];
            filled[found] = 1;
        } else if (has_k) {
            py_setitem(kdict, kwnames[k], kwvals[k]);
        } else {
            py_runtime_error("%s() got an unexpected keyword argument '%s'", fn->u.func.name, name);
        }
    }
    if (has_k) bound[np + has_v] = kdict;
    for (int p = 0; p < np; p++) {
        if (filled[p]) continue;
        int di = p - (np - fn->u.func.ndefaults);
        if (di >= 0) {
            bound[p] = fn->u.func.defaults[di];
        } else {
            py_runtime_error("%s() missing required positional argument: '%s'", fn->u.func.name,
                             fn->u.func.params[p]);
        }
    }
    Pyo* r = fn->u.func.fn(bound, nslots);
    free(bound);
    free(filled);
    return r;
}
Pyo* py_call(Pyo* fn, Pyo** args, int argc) {
    return py_call_kw(fn, args, argc, NULL, NULL, 0);
}

/* ---------------- builtins ---------------- */
Pyo* py_print(Pyo** args, int argc, Pyo* sep, Pyo* end) {
    Pyo* sepv = (sep && sep->kind != PY_NONE) ? sep : py_str(" ");
    Pyo* endv = (end && end->kind != PY_NONE) ? end : py_str("\n");
    for (int i = 0; i < argc; i++) {
        if (i) fwrite(sepv->u.str.s, 1, (size_t)sepv->u.str.len, stdout);
        Pyo* s = to_string(args[i], 0);
        fwrite(s->u.str.s, 1, (size_t)s->u.str.len, stdout);
    }
    fwrite(endv->u.str.s, 1, (size_t)endv->u.str.len, stdout);
    return py_none();
}

Pyo* py_str_concat(Pyo** items, int n) {
    SB b;
    sb_init(&b);
    for (int i = 0; i < n; i++) sb_putn(&b, items[i]->u.str.s, (size_t)items[i]->u.str.len);
    Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
    free(b.data);
    return o;
}

Pyo* py_builtin_int(Pyo* x) {
    if (x->kind == PY_INT) return x;
    if (x->kind == PY_BOOL) return py_int(x->u.b);
    if (x->kind == PY_FLOAT) return py_int((long long)x->u.f);
    if (x->kind == PY_STR) {
        const char* s = x->u.str.s;
        char* endp = NULL;
        long long v = strtoll(s, &endp, 10);
        if (endp == s) py_runtime_error("invalid literal for int(): %s", s);
        return py_int(v);
    }
    py_runtime_error("int() argument must be a string or a number");
    return py_none();
}
Pyo* py_builtin_float(Pyo* x) {
    if (x->kind == PY_FLOAT) return x;
    if (x->kind == PY_INT) return py_float((double)x->u.i);
    if (x->kind == PY_BOOL) return py_float((double)x->u.b);
    if (x->kind == PY_STR) {
        char* endp = NULL;
        double v = strtod(x->u.str.s, &endp);
        if (endp == x->u.str.s) py_runtime_error("could not convert string to float");
        return py_float(v);
    }
    py_runtime_error("float() argument must be a string or a number");
    return py_none();
}
Pyo* py_builtin_bool(Pyo* x) { return py_bool(py_truth(x)); }
Pyo* py_abs(Pyo* x) {
    if (x->kind == PY_INT) return py_int(x->u.i < 0 ? -x->u.i : x->u.i);
    if (x->kind == PY_FLOAT) return py_float(fabs(x->u.f));
    if (x->kind == PY_BOOL) return py_int(x->u.b);
    py_runtime_error("bad operand type for abs(): '%s'", kind_name(x->kind));
    return py_none();
}

Pyo* py_range(Pyo** args, int argc) {
    Pyo* o = py_new(PY_RANGE);
    long long a = 0, b = 0, c = 1;
    if (argc == 1) {
        b = as_int(args[0]);
    } else if (argc == 2) {
        a = as_int(args[0]);
        b = as_int(args[1]);
    } else if (argc == 3) {
        a = as_int(args[0]);
        b = as_int(args[1]);
        c = as_int(args[2]);
        if (c == 0) py_runtime_error("range() arg 3 must not be zero");
    } else {
        py_runtime_error("range expected 1 to 3 arguments, got %d", argc);
    }
    o->u.range.start = a;
    o->u.range.stop = b;
    o->u.range.step = c;
    return o;
}

Pyo* py_min(Pyo** args, int argc) {
    if (argc == 1) {
        Pyo* it = py_iter(args[0]);
        Pyo* best = NULL;
        Pyo* v;
        while (py_iter_next(it, &v))
            if (!best || py_less(v, best)) best = v;
        if (!best) py_runtime_error("min() arg is an empty sequence");
        return best;
    }
    Pyo* best = args[0];
    for (int i = 1; i < argc; i++)
        if (py_less(args[i], best)) best = args[i];
    return best;
}
Pyo* py_max(Pyo** args, int argc) {
    if (argc == 1) {
        Pyo* it = py_iter(args[0]);
        Pyo* best = NULL;
        Pyo* v;
        while (py_iter_next(it, &v))
            if (!best || py_less(best, v)) best = v;
        if (!best) py_runtime_error("max() arg is an empty sequence");
        return best;
    }
    Pyo* best = args[0];
    for (int i = 1; i < argc; i++)
        if (py_less(best, args[i])) best = args[i];
    return best;
}
Pyo* py_sum(Pyo** args, int argc) {
    Pyo* total = argc > 1 ? args[1] : py_int(0);
    Pyo* it = py_iter(args[0]);
    Pyo* v;
    while (py_iter_next(it, &v)) total = py_add(total, v);
    return total;
}
Pyo* py_list_of(Pyo* x) {
    if (!x || x->kind == PY_NONE) return py_list(NULL, 0);
    Pyo* it = py_iter(x);
    Pyo* o = py_list(NULL, 0);
    Pyo* v;
    while (py_iter_next(it, &v)) list_push(o, v);
    return o;
}
Pyo* py_tuple_of(Pyo* x) {
    Pyo* l = py_list_of(x);
    return py_tuple(l->u.list.items, (int)l->u.list.len);
}
static void set_add(Pyo* o, Pyo* v) {
    for (long long i = 0; i < o->u.set.len; i++)
        if (py_equal(o->u.set.items[i], v)) return;
    set_reserve(o, 1);
    o->u.set.items[o->u.set.len++] = v;
}
Pyo* py_set_of(Pyo* x) {
    Pyo* o = py_new(PY_SET);
    if (!x || x->kind == PY_NONE) return o;
    Pyo* it = py_iter(x);
    Pyo* v;
    while (py_iter_next(it, &v)) set_add(o, v);
    return o;
}
Pyo* py_dict_of(Pyo* x) {
    Pyo* o = py_new(PY_DICT);
    if (!x || x->kind == PY_NONE) return o;
    Pyo* it = py_iter(x);
    Pyo* v;
    while (py_iter_next(it, &v)) {
        if (v->kind != PY_LIST && v->kind != PY_TUPLE) py_runtime_error("dict() expects pairs");
        long long n = v->kind == PY_LIST ? v->u.list.len : v->u.tuple.len;
        Pyo** items = v->kind == PY_LIST ? v->u.list.items : v->u.tuple.items;
        if (n != 2) py_runtime_error("dict() sequence elements must have length 2");
        py_setitem(o, items[0], items[1]);
    }
    return o;
}

static int cmp_asc(const void* a, const void* b) {
    Pyo* x = *(Pyo**)a;
    Pyo* y = *(Pyo**)b;
    if (py_equal(x, y)) return 0;
    return py_less(x, y) ? -1 : 1;
}
typedef struct {
    Pyo* key;
    Pyo* val;
    long long idx;
} KItem;

static int cmp_kitem(const void* pa, const void* pb) {
    const KItem* a = (const KItem*)pa;
    const KItem* b = (const KItem*)pb;
    if (!py_equal(a->key, b->key)) return py_less(a->key, b->key) ? -1 : 1;
    if (a->idx < b->idx) return -1;
    if (a->idx > b->idx) return 1;
    return 0;
}

Pyo* py_sorted(Pyo* x, Pyo* reverse, Pyo* key) {
    Pyo* l = py_list_of(x);
    if (key && key->kind != PY_NONE) {
        long long n = l->u.list.len;
        KItem* arr = n ? (KItem*)malloc((size_t)n * sizeof(KItem)) : NULL;
        for (long long i = 0; i < n; i++) {
            arr[i].val = l->u.list.items[i];
            arr[i].idx = i;
            Pyo* one[1] = {arr[i].val};
            arr[i].key = py_call_kw(key, one, 1, NULL, NULL, 0);
        }
        qsort(arr, (size_t)n, sizeof(KItem), cmp_kitem);
        for (long long i = 0; i < n; i++) l->u.list.items[i] = arr[i].val;
        free(arr);
    } else {
        qsort(l->u.list.items, (size_t)l->u.list.len, sizeof(Pyo*), cmp_asc);
    }
    if (reverse && py_truth(reverse)) {
        for (long long i = 0, j = l->u.list.len - 1; i < j; i++, j--) {
            Pyo* t = l->u.list.items[i];
            l->u.list.items[i] = l->u.list.items[j];
            l->u.list.items[j] = t;
        }
    }
    return l;
}
Pyo* py_enumerate(Pyo* x, Pyo* start) {
    long long i = start ? as_int(start) : 0;
    Pyo* it = py_iter(x);
    Pyo* o = py_list(NULL, 0);
    Pyo* v;
    while (py_iter_next(it, &v)) {
        Pyo* pair[2] = {py_int(i++), v};
        list_push(o, py_tuple(pair, 2));
    }
    return o;
}
Pyo* py_zip(Pyo** args, int argc) {
    Pyo* o = py_list(NULL, 0);
    if (argc == 0) return o;
    Pyo** its = (Pyo**)malloc((size_t)argc * sizeof(Pyo*));
    for (int i = 0; i < argc; i++) its[i] = py_iter(args[i]);
    for (;;) {
        Pyo** vals = (Pyo**)malloc((size_t)argc * sizeof(Pyo*));
        int ok = 1;
        for (int i = 0; i < argc; i++)
            if (!py_iter_next(its[i], &vals[i])) ok = 0;
        if (!ok) {
            free(vals);
            break;
        }
        list_push(o, py_tuple(vals, argc));
        free(vals);
    }
    free(its);
    return o;
}
Pyo* py_reversed(Pyo* x) {
    Pyo* l = py_list_of(x);
    Pyo* o = py_list(NULL, 0);
    for (long long i = l->u.list.len - 1; i >= 0; i--) list_push(o, l->u.list.items[i]);
    return o;
}
Pyo* py_any(Pyo** args, int argc) {
    for (int i = 0; i < argc; i++) {
        Pyo* it = py_iter(args[i]);
        Pyo* v;
        while (py_iter_next(it, &v))
            if (py_truth(v)) return py_true();
    }
    return py_false();
}
Pyo* py_all(Pyo** args, int argc) {
    for (int i = 0; i < argc; i++) {
        Pyo* it = py_iter(args[i]);
        Pyo* v;
        while (py_iter_next(it, &v))
            if (!py_truth(v)) return py_false();
    }
    return py_true();
}
Pyo* py_isinstance(Pyo* x, Pyo* t) {
    const char* name = t->kind == PY_STR ? t->u.str.s : (t->kind == PY_FUNC ? t->u.func.name : "");
    const char* k = kind_name(x->kind);
    if (strcmp(name, k) == 0) return py_true();
    if (strcmp(name, "object") == 0) return py_true();
    if (strcmp(name, "int") == 0 && x->kind == PY_BOOL) return py_true();
    if (strcmp(name, "number") == 0 && is_num(x)) return py_true();
    return py_false();
}
Pyo* py_round(Pyo** args, int argc) {
    Pyo* x = args[0];
    int nd = argc > 1 ? (int)as_int(args[1]) : 0;
    if (x->kind == PY_INT) return x;
    double v = as_double(x);
    double scale = pow(10.0, nd);
    double r = round(v * scale) / scale;
    if (argc <= 1) {
        if (isnan(r) || isinf(r)) return x;
        return py_int((long long)nearbyint(v));
    }
    return py_float(r);
}
Pyo* py_chr(Pyo* x) {
    long long c = as_int(x);
    char buf[8];
    int n = 0;
    if (c < 0x80) {
        buf[n++] = (char)c;
    } else if (c < 0x800) {
        buf[n++] = (char)(0xC0 | (c >> 6));
        buf[n++] = (char)(0x80 | (c & 0x3F));
    } else if (c < 0x10000) {
        buf[n++] = (char)(0xE0 | (c >> 12));
        buf[n++] = (char)(0x80 | ((c >> 6) & 0x3F));
        buf[n++] = (char)(0x80 | (c & 0x3F));
    } else {
        buf[n++] = (char)(0xF0 | (c >> 18));
        buf[n++] = (char)(0x80 | ((c >> 12) & 0x3F));
        buf[n++] = (char)(0x80 | ((c >> 6) & 0x3F));
        buf[n++] = (char)(0x80 | (c & 0x3F));
    }
    return py_strn(buf, n);
}
Pyo* py_ord(Pyo* x) {
    if (x->kind != PY_STR || x->u.str.len < 1) py_runtime_error("ord() expected a character");
    unsigned char c = (unsigned char)x->u.str.s[0];
    return py_int(c);
}
Pyo* py_hex(Pyo* x) {
    char buf[32];
    snprintf(buf, sizeof(buf), "0x%llx", (unsigned long long)as_int(x));
    return py_str(buf);
}
Pyo* py_oct(Pyo* x) {
    char buf[32];
    snprintf(buf, sizeof(buf), "0o%llo", (unsigned long long)as_int(x));
    return py_str(buf);
}
Pyo* py_bin(Pyo* x) {
    char buf[80];
    unsigned long long v = (unsigned long long)as_int(x);
    int i = 0;
    if (v == 0) {
        return py_str("0b0");
    }
    char tmp[70];
    while (v) {
        tmp[i++] = (char)('0' + (v & 1));
        v >>= 1;
    }
    int j = 0;
    buf[j++] = '0';
    buf[j++] = 'b';
    while (i > 0) buf[j++] = tmp[--i];
    buf[j] = 0;
    return py_str(buf);
}
Pyo* py_divmod(Pyo* a, Pyo* b) {
    Pyo* pair[2] = {py_floordiv(a, b), py_mod(a, b)};
    return py_tuple(pair, 2);
}
static Pyo* py_call_builtin(const char* name, Pyo** args, int argc, Pyo** kwnames, Pyo** kwvals,
                            int nkw) {
    (void)kwnames;
    (void)kwvals;
    if (strcmp(name, "len") == 0) {
        if (argc != 1) py_runtime_error("len() takes exactly one argument");
        return py_len(args[0]);
    }
    if (strcmp(name, "abs") == 0) {
        if (argc != 1) py_runtime_error("abs() takes exactly one argument");
        return py_abs(args[0]);
    }
    if (strcmp(name, "int") == 0) {
        if (argc != 1) py_runtime_error("int() takes exactly one argument");
        return py_builtin_int(args[0]);
    }
    if (strcmp(name, "float") == 0) {
        if (argc != 1) py_runtime_error("float() takes exactly one argument");
        return py_builtin_float(args[0]);
    }
    if (strcmp(name, "str") == 0) {
        if (argc != 1) py_runtime_error("str() takes exactly one argument");
        return py_builtin_str(args[0]);
    }
    if (strcmp(name, "repr") == 0) {
        if (argc != 1) py_runtime_error("repr() takes exactly one argument");
        return py_builtin_repr(args[0]);
    }
    if (strcmp(name, "bool") == 0) {
        if (argc != 1) py_runtime_error("bool() takes exactly one argument");
        return py_builtin_bool(args[0]);
    }
    if (strcmp(name, "sum") == 0) return py_sum(args, argc);
    if (strcmp(name, "min") == 0) return py_min(args, argc);
    if (strcmp(name, "max") == 0) return py_max(args, argc);
    if (strcmp(name, "round") == 0) return py_round(args, argc);
    if (nkw > 0) py_runtime_error("keyword arguments are not supported for builtin callable '%s'", name);
    py_runtime_error("builtin '%s' cannot be used as a value", name);
    return py_none();
}
Pyo* py_input(Pyo** args, int argc) {
    if (argc > 0) {
        Pyo* p = to_string(args[0], 0);
        fwrite(p->u.str.s, 1, (size_t)p->u.str.len, stdout);
        fflush(stdout);
    }
    SB b;
    sb_init(&b);
    int c;
    while ((c = fgetc(stdin)) != EOF && c != '\n') sb_putc(&b, (char)c);
    if (c == EOF && b.len == 0) {
        free(b.data);
        return py_none();
    }
    Pyo* o = py_strn(b.data ? b.data : "", (long long)b.len);
    free(b.data);
    return o;
}

/* ---------------- f-string formatting ---------------- */
Pyo* py_format(Pyo* value, const char* spec, int repr) {
    if (!spec || spec[0] == '\0') return to_string(value, repr);
    /* Parse a small subset of the format mini-language. */
    char align = 0;
    char fill = ' ';
    int zero = 0;
    int comma = 0;
    int plus = 0;
    int width = 0;
    int precision = -1;
    char type = 0;
    const char* p = spec;
    if (p[0] && p[1] && strchr("<>=^", p[1])) {
        fill = p[0];
        align = p[1];
        p += 2;
    } else if (p[0] && strchr("<>^=", p[0])) {
        align = p[0];
        p += 1;
    }
    while (*p == '+' || *p == '-' || *p == ' ' || *p == '#' || *p == '0' || *p == ',') {
        if (*p == '0') zero = 1;
        if (*p == ',') comma = 1;
        if (*p == '+') plus = 1;
        p++;
    }
    while (isdigit((unsigned char)*p)) {
        width = width * 10 + (*p - '0');
        p++;
    }
    if (*p == '.') {
        p++;
        precision = 0;
        while (isdigit((unsigned char)*p)) {
            precision = precision * 10 + (*p - '0');
            p++;
        }
    }
    if (*p) type = *p;
    char num[128];
    char body[160];
    if (type == 'f' || type == 'F' || type == 'e' || type == 'E' || type == 'g' || type == 'G' ||
        (value->kind == PY_FLOAT && type == 0)) {
        double d = as_double(value);
        int prec = precision >= 0 ? precision : 6;
        if (type == 'e' || type == 'E') {
            snprintf(num, sizeof(num), "%.*e", prec, d);
        } else if (type == 'g' || type == 'G') {
            snprintf(num, sizeof(num), "%.*g", prec, d);
        } else {
            snprintf(num, sizeof(num), "%.*f", prec, d);
        }
        if (precision < 0 && type == 'f') {
            /* default f formatting uses 6 decimals, matches Python */
        }
    } else if (type == 'd') {
        snprintf(num, sizeof(num), plus ? "%+lld" : "%lld", as_int(value));
    } else if (type == 'x') {
        snprintf(num, sizeof(num), "%llx", (unsigned long long)as_int(value));
    } else if (type == 'X') {
        snprintf(num, sizeof(num), "%llX", (unsigned long long)as_int(value));
    } else if (type == 'o') {
        snprintf(num, sizeof(num), "%llo", (unsigned long long)as_int(value));
    } else if (type == 'b') {
        Pyo* s = py_bin(value);
        snprintf(num, sizeof(num), "%.*s", (int)s->u.str.len - 2, s->u.str.s + 2);
    } else if (type == 'c') {
        snprintf(num, sizeof(num), "%c", (int)as_int(value));
    } else {
        Pyo* s = to_string(value, repr);
        long long n = s->u.str.len < (long long)sizeof(num) - 1 ? s->u.str.len : (long long)sizeof(num) - 1;
        memcpy(num, s->u.str.s, (size_t)n);
        num[n] = 0;
    }
    if (comma && (type == 'd' || type == 0)) {
        /* insert thousands separators */
        char tmp[160];
        int n = (int)strlen(num);
        int neg = 0, j = 0;
        if (num[0] == '-') {
            neg = 1;
            n--;
        }
        char digits[160];
        memcpy(digits, num + neg, (size_t)n);
        digits[n] = 0;
        for (int i = 0; i < n; i++) {
            tmp[j++] = digits[i];
            if ((n - i - 1) % 3 == 0 && i != n - 1) tmp[j++] = ',';
        }
        tmp[j] = 0;
        if (neg) {
            memmove(tmp + 1, tmp, (size_t)j + 1);
            tmp[0] = '-';
        }
        snprintf(body, sizeof(body), "%s", tmp);
    } else {
        snprintf(body, sizeof(body), "%s", num);
    }
    int blen = (int)strlen(body);
    if (width <= blen) return py_str(body);
    char out[512];
    int pad = width - blen;
    char use_align = align;
    if (use_align == 0) use_align = (value->kind == PY_INT || value->kind == PY_FLOAT) ? '>' : '<';
    char padchar = zero && use_align == '>' && (value->kind == PY_INT || value->kind == PY_FLOAT) ? '0' : fill;
    int idx = 0;
    if (use_align == '<') {
        memcpy(out, body, (size_t)blen);
        idx = blen;
        for (int i = 0; i < pad; i++) out[idx++] = fill;
    } else if (use_align == '>') {
        for (int i = 0; i < pad; i++) out[idx++] = padchar;
        memcpy(out + idx, body, (size_t)blen);
        idx += blen;
    } else { /* ^ */
        int left = pad / 2, right = pad - left;
        for (int i = 0; i < left; i++) out[idx++] = fill;
        memcpy(out + idx, body, (size_t)blen);
        idx += blen;
        for (int i = 0; i < right; i++) out[idx++] = fill;
    }
    out[idx] = 0;
    return py_strn(out, idx);
}

/* ---------------- methods ---------------- */
static Pyo* list_method(Pyo* self, const char* name, Pyo** args, int argc) {
    if (strcmp(name, "append") == 0) {
        list_push(self, args[0]);
        return py_none();
    }
    if (strcmp(name, "extend") == 0) {
        Pyo* it = py_iter(args[0]);
        Pyo* v;
        while (py_iter_next(it, &v)) list_push(self, v);
        return py_none();
    }
    if (strcmp(name, "insert") == 0) {
        long long i = as_int(args[0]);
        if (i < 0) i += self->u.list.len;
        if (i < 0) i = 0;
        if (i > self->u.list.len) i = self->u.list.len;
        list_reserve(self, 1);
        for (long long j = self->u.list.len; j > i; j--) self->u.list.items[j] = self->u.list.items[j - 1];
        self->u.list.items[i] = args[1];
        self->u.list.len++;
        return py_none();
    }
    if (strcmp(name, "pop") == 0) {
        if (self->u.list.len == 0) py_runtime_error("pop from empty list");
        long long i = argc > 0 ? as_int(args[0]) : self->u.list.len - 1;
        i = normalize_index(i, self->u.list.len);
        Pyo* v = self->u.list.items[i];
        for (long long j = i; j + 1 < self->u.list.len; j++) self->u.list.items[j] = self->u.list.items[j + 1];
        self->u.list.len--;
        return v;
    }
    if (strcmp(name, "remove") == 0) {
        for (long long i = 0; i < self->u.list.len; i++) {
            if (py_equal(self->u.list.items[i], args[0])) {
                for (long long j = i; j + 1 < self->u.list.len; j++) self->u.list.items[j] = self->u.list.items[j + 1];
                self->u.list.len--;
                return py_none();
            }
        }
        py_runtime_error("list.remove(x): x not in list");
    }
    if (strcmp(name, "index") == 0) {
        for (long long i = 0; i < self->u.list.len; i++)
            if (py_equal(self->u.list.items[i], args[0])) return py_int(i);
        py_runtime_error("value is not in list");
    }
    if (strcmp(name, "count") == 0) {
        long long c = 0;
        for (long long i = 0; i < self->u.list.len; i++)
            if (py_equal(self->u.list.items[i], args[0])) c++;
        return py_int(c);
    }
    if (strcmp(name, "sort") == 0) {
        qsort(self->u.list.items, (size_t)self->u.list.len, sizeof(Pyo*), cmp_asc);
        return py_none();
    }
    if (strcmp(name, "reverse") == 0) {
        for (long long i = 0, j = self->u.list.len - 1; i < j; i++, j--) {
            Pyo* t = self->u.list.items[i];
            self->u.list.items[i] = self->u.list.items[j];
            self->u.list.items[j] = t;
        }
        return py_none();
    }
    if (strcmp(name, "clear") == 0) {
        self->u.list.len = 0;
        return py_none();
    }
    if (strcmp(name, "copy") == 0) return py_list(self->u.list.items, (int)self->u.list.len);
    py_runtime_error("'list' object has no attribute '%s'", name);
    return py_none();
}

static Pyo* dict_get(Pyo* self, Pyo* key) {
    for (long long i = 0; i < self->u.dict.len; i++)
        if (py_equal(self->u.dict.keys[i], key)) return self->u.dict.vals[i];
    return NULL;
}

static Pyo* dict_method(Pyo* self, const char* name, Pyo** args, int argc) {
    if (strcmp(name, "get") == 0) {
        Pyo* v = dict_get(self, args[0]);
        if (v) return v;
        return argc > 1 ? args[1] : py_none();
    }
    if (strcmp(name, "keys") == 0) return py_list(self->u.dict.keys, (int)self->u.dict.len);
    if (strcmp(name, "values") == 0) return py_list(self->u.dict.vals, (int)self->u.dict.len);
    if (strcmp(name, "items") == 0) {
        Pyo* o = py_list(NULL, 0);
        for (long long i = 0; i < self->u.dict.len; i++) {
            Pyo* pair[2] = {self->u.dict.keys[i], self->u.dict.vals[i]};
            list_push(o, py_tuple(pair, 2));
        }
        return o;
    }
    if (strcmp(name, "pop") == 0) {
        for (long long i = 0; i < self->u.dict.len; i++) {
            if (py_equal(self->u.dict.keys[i], args[0])) {
                Pyo* v = self->u.dict.vals[i];
                for (long long j = i; j + 1 < self->u.dict.len; j++) {
                    self->u.dict.keys[j] = self->u.dict.keys[j + 1];
                    self->u.dict.vals[j] = self->u.dict.vals[j + 1];
                }
                self->u.dict.len--;
                return v;
            }
        }
        if (argc > 1) return args[1];
        py_runtime_error("KeyError");
    }
    if (strcmp(name, "setdefault") == 0) {
        Pyo* v = dict_get(self, args[0]);
        if (v) return v;
        Pyo* d = argc > 1 ? args[1] : py_none();
        py_setitem(self, args[0], d);
        return d;
    }
    if (strcmp(name, "update") == 0) {
        if (args[0]->kind == PY_DICT) {
            for (long long i = 0; i < args[0]->u.dict.len; i++)
                py_setitem(self, args[0]->u.dict.keys[i], args[0]->u.dict.vals[i]);
            return py_none();
        }
        Pyo* it = py_iter(args[0]);
        Pyo* v;
        while (py_iter_next(it, &v)) {
            if (v->kind != PY_LIST && v->kind != PY_TUPLE)
                py_runtime_error("cannot convert dictionary update sequence element to a sequence");
            long long n = v->kind == PY_LIST ? v->u.list.len : v->u.tuple.len;
            Pyo** items = v->kind == PY_LIST ? v->u.list.items : v->u.tuple.items;
            if (n != 2) py_runtime_error("dictionary update sequence element has length != 2");
            py_setitem(self, items[0], items[1]);
        }
        return py_none();
    }
    if (strcmp(name, "clear") == 0) {
        self->u.dict.len = 0;
        return py_none();
    }
    if (strcmp(name, "copy") == 0)
        return py_dict(self->u.dict.keys, self->u.dict.vals, (int)self->u.dict.len);
    py_runtime_error("'dict' object has no attribute '%s'", name);
    return py_none();
}

static int str_starts_with(Pyo* s, Pyo* prefix) {
    return prefix->u.str.len <= s->u.str.len &&
           memcmp(s->u.str.s, prefix->u.str.s, (size_t)prefix->u.str.len) == 0;
}
static int str_ends_with(Pyo* s, Pyo* suffix) {
    return suffix->u.str.len <= s->u.str.len &&
           memcmp(s->u.str.s + (s->u.str.len - suffix->u.str.len), suffix->u.str.s,
                  (size_t)suffix->u.str.len) == 0;
}
static long long str_find(Pyo* s, Pyo* sub, long long start) {
    if (sub->u.str.len == 0) return start <= s->u.str.len ? start : -1;
    for (long long i = start; i + sub->u.str.len <= s->u.str.len; i++)
        if (memcmp(s->u.str.s + i, sub->u.str.s, (size_t)sub->u.str.len) == 0) return i;
    return -1;
}

static Pyo* str_split_impl(Pyo* s, Pyo* sep, int maxsplit, int from_right) {
    Pyo* o = py_list(NULL, 0);
    if (!sep || sep->kind == PY_NONE) {
        long long i = 0;
        while (i < s->u.str.len) {
            while (i < s->u.str.len && isspace((unsigned char)s->u.str.s[i])) i++;
            if (i >= s->u.str.len) break;
            long long j = i;
            while (j < s->u.str.len && !isspace((unsigned char)s->u.str.s[j])) j++;
            list_push(o, py_strn(s->u.str.s + i, j - i));
            i = j;
        }
        return o;
    }
    if (sep->u.str.len == 0) py_runtime_error("empty separator");
    long long start = 0, splits = 0;
    (void)from_right;
    for (long long i = 0; i + sep->u.str.len <= s->u.str.len;) {
        if ((maxsplit < 0 || splits < maxsplit) &&
            memcmp(s->u.str.s + i, sep->u.str.s, (size_t)sep->u.str.len) == 0) {
            list_push(o, py_strn(s->u.str.s + start, i - start));
            i += sep->u.str.len;
            start = i;
            splits++;
        } else {
            i++;
        }
    }
    list_push(o, py_strn(s->u.str.s + start, s->u.str.len - start));
    return o;
}

static Pyo* str_method(Pyo* self, const char* name, Pyo** args, int argc) {
    if (strcmp(name, "upper") == 0 || strcmp(name, "lower") == 0 ||
        strcmp(name, "capitalize") == 0 || strcmp(name, "swapcase") == 0) {
        char* buf = (char*)malloc((size_t)self->u.str.len + 1);
        int upper = strcmp(name, "upper") == 0;
        int cap = strcmp(name, "capitalize") == 0;
        int swap = strcmp(name, "swapcase") == 0;
        for (long long i = 0; i < self->u.str.len; i++) {
            unsigned char c = (unsigned char)self->u.str.s[i];
            if (cap) buf[i] = i == 0 ? (char)toupper(c) : (char)tolower(c);
            else if (swap) buf[i] = (char)(isupper(c) ? tolower(c) : toupper(c));
            else buf[i] = (char)(upper ? toupper(c) : tolower(c));
        }
        buf[self->u.str.len] = 0;
        Pyo* o = py_strn(buf, self->u.str.len);
        free(buf);
        return o;
    }
    if (strcmp(name, "strip") == 0 || strcmp(name, "lstrip") == 0 || strcmp(name, "rstrip") == 0) {
        long long a = 0, b = self->u.str.len;
        int left = strcmp(name, "rstrip") != 0;
        int right = strcmp(name, "lstrip") != 0;
        if (left)
            while (a < b && isspace((unsigned char)self->u.str.s[a])) a++;
        if (right)
            while (b > a && isspace((unsigned char)self->u.str.s[b - 1])) b--;
        return py_strn(self->u.str.s + a, b - a);
    }
    if (strcmp(name, "split") == 0 || strcmp(name, "rsplit") == 0) {
        Pyo* sep = argc > 0 ? args[0] : NULL;
        int maxsplit = argc > 1 ? (int)as_int(args[1]) : -1;
        return str_split_impl(self, sep, maxsplit, strcmp(name, "rsplit") == 0);
    }
    if (strcmp(name, "splitlines") == 0) {
        Pyo* o = py_list(NULL, 0);
        long long start = 0;
        for (long long i = 0; i < self->u.str.len; i++) {
            if (self->u.str.s[i] == '\n') {
                long long end = (i > start && self->u.str.s[i - 1] == '\r') ? i - 1 : i;
                list_push(o, py_strn(self->u.str.s + start, end - start));
                start = i + 1;
            }
        }
        if (start < self->u.str.len) list_push(o, py_strn(self->u.str.s + start, self->u.str.len - start));
        return o;
    }
    if (strcmp(name, "join") == 0) {
        SB b;
        sb_init(&b);
        Pyo* it = py_iter(args[0]);
        Pyo* v;
        int first = 1;
        while (py_iter_next(it, &v)) {
            if (v->kind != PY_STR) py_runtime_error("sequence item: expected str instance");
            if (!first) sb_putn(&b, self->u.str.s, (size_t)self->u.str.len);
            sb_putn(&b, v->u.str.s, (size_t)v->u.str.len);
            first = 0;
        }
        Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
        free(b.data);
        return o;
    }
    if (strcmp(name, "replace") == 0) {
        Pyo* from = args[0];
        Pyo* to = args[1];
        SB b;
        sb_init(&b);
        long long i = 0;
        if (from->u.str.len == 0) return self;
        while (i < self->u.str.len) {
            if (i + from->u.str.len <= self->u.str.len &&
                memcmp(self->u.str.s + i, from->u.str.s, (size_t)from->u.str.len) == 0) {
                sb_putn(&b, to->u.str.s, (size_t)to->u.str.len);
                i += from->u.str.len;
            } else {
                sb_putc(&b, self->u.str.s[i]);
                i++;
            }
        }
        Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
        free(b.data);
        return o;
    }
    if (strcmp(name, "find") == 0 || strcmp(name, "rfind") == 0) {
        if (strcmp(name, "find") == 0) return py_int(str_find(self, args[0], 0));
        long long best = -1;
        for (long long i = 0; i + args[0]->u.str.len <= self->u.str.len; i++)
            if (memcmp(self->u.str.s + i, args[0]->u.str.s, (size_t)args[0]->u.str.len) == 0) best = i;
        return py_int(best);
    }
    if (strcmp(name, "index") == 0) {
        long long i = str_find(self, args[0], 0);
        if (i < 0) py_runtime_error("substring not found");
        return py_int(i);
    }
    if (strcmp(name, "count") == 0) {
        long long c = 0;
        if (args[0]->u.str.len == 0) return py_int(self->u.str.len + 1);
        for (long long i = 0; i + args[0]->u.str.len <= self->u.str.len; i++)
            if (memcmp(self->u.str.s + i, args[0]->u.str.s, (size_t)args[0]->u.str.len) == 0) c++;
        return py_int(c);
    }
    if (strcmp(name, "startswith") == 0) return py_bool(str_starts_with(self, args[0]));
    if (strcmp(name, "endswith") == 0) return py_bool(str_ends_with(self, args[0]));
    if (strcmp(name, "isdigit") == 0 || strcmp(name, "isnumeric") == 0 ||
        strcmp(name, "isdecimal") == 0) {
        if (self->u.str.len == 0) return py_false();
        for (long long i = 0; i < self->u.str.len; i++)
            if (!isdigit((unsigned char)self->u.str.s[i])) return py_false();
        return py_true();
    }
    if (strcmp(name, "isalpha") == 0) {
        if (self->u.str.len == 0) return py_false();
        for (long long i = 0; i < self->u.str.len; i++)
            if (!isalpha((unsigned char)self->u.str.s[i])) return py_false();
        return py_true();
    }
    if (strcmp(name, "isspace") == 0) {
        if (self->u.str.len == 0) return py_false();
        for (long long i = 0; i < self->u.str.len; i++)
            if (!isspace((unsigned char)self->u.str.s[i])) return py_false();
        return py_true();
    }
    if (strcmp(name, "isupper") == 0) {
        int any = 0;
        for (long long i = 0; i < self->u.str.len; i++) {
            if (islower((unsigned char)self->u.str.s[i])) return py_false();
            if (isupper((unsigned char)self->u.str.s[i])) any = 1;
        }
        return py_bool(any);
    }
    if (strcmp(name, "islower") == 0) {
        int any = 0;
        for (long long i = 0; i < self->u.str.len; i++) {
            if (isupper((unsigned char)self->u.str.s[i])) return py_false();
            if (islower((unsigned char)self->u.str.s[i])) any = 1;
        }
        return py_bool(any);
    }
    if (strcmp(name, "zfill") == 0) {
        long long w = as_int(args[0]);
        if (w <= self->u.str.len) return self;
        SB b;
        sb_init(&b);
        long long pad = w - self->u.str.len;
        int neg = self->u.str.len > 0 && (self->u.str.s[0] == '-' || self->u.str.s[0] == '+');
        if (neg) sb_putc(&b, self->u.str.s[0]);
        for (long long i = 0; i < pad; i++) sb_putc(&b, '0');
        sb_putn(&b, self->u.str.s + (neg ? 1 : 0), (size_t)(self->u.str.len - (neg ? 1 : 0)));
        Pyo* o = py_strn(b.data ? b.data : "", b.data ? (long long)b.len : 0);
        free(b.data);
        return o;
    }
    if (strcmp(name, "title") == 0) {
        char* buf = (char*)malloc((size_t)self->u.str.len + 1);
        int new_word = 1;
        for (long long i = 0; i < self->u.str.len; i++) {
            unsigned char c = (unsigned char)self->u.str.s[i];
            if (isalpha(c)) {
                buf[i] = (char)(new_word ? toupper(c) : tolower(c));
                new_word = 0;
            } else {
                buf[i] = (char)c;
                new_word = 1;
            }
        }
        buf[self->u.str.len] = 0;
        Pyo* o = py_strn(buf, self->u.str.len);
        free(buf);
        return o;
    }
    py_runtime_error("'str' object has no attribute '%s'", name);
    return py_none();
}

static Pyo* set_method(Pyo* self, const char* name, Pyo** args, int argc) {
    (void)argc;
    if (strcmp(name, "add") == 0) {
        set_add(self, args[0]);
        return py_none();
    }
    if (strcmp(name, "discard") == 0) {
        for (long long i = 0; i < self->u.set.len; i++)
            if (py_equal(self->u.set.items[i], args[0])) {
                for (long long j = i; j + 1 < self->u.set.len; j++) self->u.set.items[j] = self->u.set.items[j + 1];
                self->u.set.len--;
                return py_none();
            }
        return py_none();
    }
    if (strcmp(name, "remove") == 0) {
        for (long long i = 0; i < self->u.set.len; i++)
            if (py_equal(self->u.set.items[i], args[0])) {
                for (long long j = i; j + 1 < self->u.set.len; j++) self->u.set.items[j] = self->u.set.items[j + 1];
                self->u.set.len--;
                return py_none();
            }
        py_runtime_error("KeyError");
    }
    if (strcmp(name, "clear") == 0) {
        self->u.set.len = 0;
        return py_none();
    }
    if (strcmp(name, "update") == 0) {
        Pyo* it = py_iter(args[0]);
        Pyo* v;
        while (py_iter_next(it, &v)) set_add(self, v);
        return py_none();
    }
    if (strcmp(name, "copy") == 0) return py_set_of(self);
    if (strcmp(name, "union") == 0) {
        Pyo* o = py_set_of(self);
        for (int i = 0; i < argc; i++) {
            Pyo* it = py_iter(args[i]);
            Pyo* v;
            while (py_iter_next(it, &v)) set_add(o, v);
        }
        return o;
    }
    if (strcmp(name, "intersection") == 0) {
        Pyo* o = py_set_of(self);
        Pyo* keep = py_new(PY_SET);
        for (long long i = 0; i < o->u.set.len; i++) {
            int inall = 1;
            for (int a = 0; a < argc; a++)
                if (!py_truth(py_in(o->u.set.items[i], args[a]))) inall = 0;
            if (inall) set_add(keep, o->u.set.items[i]);
        }
        return keep;
    }
    if (strcmp(name, "difference") == 0) {
        Pyo* o = py_set_of(self);
        for (int a = 0; a < argc; a++) {
            Pyo* res = py_new(PY_SET);
            for (long long i = 0; i < o->u.set.len; i++)
                if (!py_truth(py_in(o->u.set.items[i], args[a]))) set_add(res, o->u.set.items[i]);
            o = res;
        }
        return o;
    }
    if (strcmp(name, "issubset") == 0) {
        for (long long i = 0; i < self->u.set.len; i++)
            if (!py_truth(py_in(self->u.set.items[i], args[0]))) return py_false();
        return py_true();
    }
    if (strcmp(name, "issuperset") == 0) {
        Pyo* other = args[0];
        for (long long i = 0; i < other->u.set.len; i++)
            if (!py_truth(py_in(other->u.set.items[i], self))) return py_false();
        return py_true();
    }
    py_runtime_error("'set' object has no attribute '%s'", name);
    return py_none();
}

void py_list_sort(Pyo* list, Pyo* reverse, Pyo* key) {
    if (list->kind != PY_LIST) py_runtime_error("'%s' object has no attribute 'sort'", kind_name(list->kind));
    if (key && key->kind != PY_NONE) {
        long long n = list->u.list.len;
        KItem* arr = n ? (KItem*)malloc((size_t)n * sizeof(KItem)) : NULL;
        for (long long i = 0; i < n; i++) {
            arr[i].val = list->u.list.items[i];
            arr[i].idx = i;
            Pyo* one[1] = {arr[i].val};
            arr[i].key = py_call_kw(key, one, 1, NULL, NULL, 0);
        }
        qsort(arr, (size_t)n, sizeof(KItem), cmp_kitem);
        for (long long i = 0; i < n; i++) list->u.list.items[i] = arr[i].val;
        free(arr);
    } else {
        qsort(list->u.list.items, (size_t)list->u.list.len, sizeof(Pyo*), cmp_asc);
    }
    if (reverse && py_truth(reverse)) {
        for (long long i = 0, j = list->u.list.len - 1; i < j; i++, j--) {
            Pyo* t = list->u.list.items[i];
            list->u.list.items[i] = list->u.list.items[j];
            list->u.list.items[j] = t;
        }
    }
}

Pyo* py_method(Pyo* obj, const char* name, Pyo** args, int argc) {
    switch (obj->kind) {
        case PY_LIST: return list_method(obj, name, args, argc);
        case PY_STR: return str_method(obj, name, args, argc);
        case PY_DICT: return dict_method(obj, name, args, argc);
        case PY_SET: return set_method(obj, name, args, argc);
        case PY_TUPLE:
            if (strcmp(name, "count") == 0) {
                long long c = 0;
                for (long long i = 0; i < obj->u.tuple.len; i++)
                    if (py_equal(obj->u.tuple.items[i], args[0])) c++;
                return py_int(c);
            }
            if (strcmp(name, "index") == 0) {
                for (long long i = 0; i < obj->u.tuple.len; i++)
                    if (py_equal(obj->u.tuple.items[i], args[0])) return py_int(i);
                py_runtime_error("tuple.index(x): x not in tuple");
            }
            break;
        default:
            break;
    }
    py_runtime_error("'%s' object has no attribute '%s'", kind_name(obj->kind), name);
    return py_none();
}

/* ---------------- dynamic argument vectors ---------------- */
void py_args_init(ArgVec* v) {
    v->items = NULL;
    v->len = 0;
    v->cap = 0;
}
static void args_reserve(ArgVec* v, int extra) {
    if (v->len + extra <= v->cap) return;
    int nc = v->cap ? v->cap * 2 : 8;
    while (nc < v->len + extra) nc *= 2;
    v->items = (Pyo**)realloc(v->items, (size_t)nc * sizeof(Pyo*));
    if (!v->items) py_runtime_error("out of memory");
    v->cap = nc;
}
void py_args_push(ArgVec* v, Pyo* x) {
    args_reserve(v, 1);
    v->items[v->len++] = x;
}
void py_args_extend(ArgVec* v, Pyo* iterable) {
    Pyo* it = py_iter(iterable);
    Pyo* val;
    while (py_iter_next(it, &val)) py_args_push(v, val);
}
void py_args_extend_kv(ArgVec* names, ArgVec* vals, Pyo* dict) {
    if (dict->kind != PY_DICT) py_runtime_error("argument after ** must be a mapping");
    for (long long i = 0; i < dict->u.dict.len; i++) {
        if (dict->u.dict.keys[i]->kind != PY_STR) py_runtime_error("keywords must be strings");
        py_args_push(names, dict->u.dict.keys[i]);
        py_args_push(vals, dict->u.dict.vals[i]);
    }
}
void py_list_append(Pyo* list, Pyo* v) { list_push(list, v); }
void py_set_add(Pyo* set, Pyo* v) {
    if (set->kind != PY_SET) py_runtime_error("py_set_add expects a set");
    set_add(set, v);
}
void py_dict_set(Pyo* dict, Pyo* k, Pyo* v) { py_setitem(dict, k, v); }

/* ---------------- sys / cli ---------------- */
static int g_argc = 0;
static char** g_argv = NULL;
void py_set_argv(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
}
Pyo* py_sys_argv(void) {
    Pyo* o = py_list(NULL, 0);
    for (int i = 0; i < g_argc; i++) list_push(o, py_str(g_argv[i]));
    return o;
}
void py_sys_exit(Pyo* code) {
    fflush(stdout);
    int c = (code && code->kind != PY_NONE) ? (int)as_int(code) : 0;
    exit(c);
}

/* ---------------- math module ---------------- */
Pyo* py_math_const(const char* name) {
    if (strcmp(name, "pi") == 0) return py_float(M_PI);
    if (strcmp(name, "e") == 0) return py_float(M_E);
    if (strcmp(name, "tau") == 0) return py_float(2.0 * M_PI);
    if (strcmp(name, "inf") == 0) return py_float(INFINITY);
    if (strcmp(name, "nan") == 0) return py_float(NAN);
    py_runtime_error("math has no attribute '%s'", name);
    return py_none();
}
Pyo* py_math_call(const char* name, Pyo** args, int argc) {
    double a0 = argc > 0 ? as_double(args[0]) : 0.0;
    double a1 = argc > 1 ? as_double(args[1]) : 0.0;
    if (strcmp(name, "sqrt") == 0) return py_float(sqrt(a0));
    if (strcmp(name, "sin") == 0) return py_float(sin(a0));
    if (strcmp(name, "cos") == 0) return py_float(cos(a0));
    if (strcmp(name, "tan") == 0) return py_float(tan(a0));
    if (strcmp(name, "asin") == 0) return py_float(asin(a0));
    if (strcmp(name, "acos") == 0) return py_float(acos(a0));
    if (strcmp(name, "atan") == 0) return py_float(atan(a0));
    if (strcmp(name, "atan2") == 0) return py_float(atan2(a0, a1));
    if (strcmp(name, "exp") == 0) return py_float(exp(a0));
    if (strcmp(name, "log") == 0) return py_float(argc > 1 ? log(a0) / log(a1) : log(a0));
    if (strcmp(name, "log2") == 0) return py_float(log2(a0));
    if (strcmp(name, "log10") == 0) return py_float(log10(a0));
    if (strcmp(name, "pow") == 0) return py_float(pow(a0, a1));
    if (strcmp(name, "floor") == 0) return py_int((long long)floor(a0));
    if (strcmp(name, "ceil") == 0) return py_int((long long)ceil(a0));
    if (strcmp(name, "trunc") == 0) return py_int((long long)trunc(a0));
    if (strcmp(name, "fabs") == 0) return py_float(fabs(a0));
    if (strcmp(name, "fmod") == 0) return py_float(fmod(a0, a1));
    if (strcmp(name, "hypot") == 0) return py_float(hypot(a0, a1));
    if (strcmp(name, "radians") == 0) return py_float(a0 * M_PI / 180.0);
    if (strcmp(name, "degrees") == 0) return py_float(a0 * 180.0 / M_PI);
    if (strcmp(name, "isqrt") == 0) return py_int((long long)sqrt((double)as_int(args[0])));
    if (strcmp(name, "gcd") == 0) {
        long long x = as_int(args[0]);
        long long y = as_int(args[1]);
        if (x < 0) x = -x;
        if (y < 0) y = -y;
        while (y) {
            long long t = x % y;
            x = y;
            y = t;
        }
        return py_int(x);
    }
    if (strcmp(name, "factorial") == 0) {
        long long n = as_int(args[0]);
        long long r = 1;
        for (long long i = 2; i <= n; i++) r *= i;
        return py_int(r);
    }
    if (strcmp(name, "isnan") == 0) return py_bool(isnan(a0));
    if (strcmp(name, "isinf") == 0) return py_bool(isinf(a0));
    py_runtime_error("math has no function '%s'", name);
    return py_none();
}

void py_shutdown(void) {}
