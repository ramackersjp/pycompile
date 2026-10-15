/* pyrt.h - runtime support library for programs compiled by pycompile.
 * The generated C code includes this header and links against pyrt.c. */
#ifndef PYRT_H
#define PYRT_H

#include <setjmp.h>
#include <stddef.h>

typedef struct Pyo Pyo;
typedef Pyo* (*PyFn)(Pyo** args, int argc);

/* Exception handling */
typedef struct PyHandler {
    jmp_buf jb;
    struct PyHandler* prev;
} PyHandler;
void py_exc_push(PyHandler* h);
void py_exc_pop(void);
Pyo* py_exc_ref(const char* name);
int  py_exc_matches(Pyo* exc, Pyo* typ);
void py_raise(Pyo* exc);
void py_raise_value(Pyo* v);
void py_reraise(void);
Pyo* py_exc_current(void);
void py_exc_set(Pyo* exc);

/* Constructors */
Pyo* py_none(void);
Pyo* py_true(void);
Pyo* py_false(void);
Pyo* py_bool(int b);
Pyo* py_int(long long v);
Pyo* py_float(double v);
Pyo* py_str(const char* s);
Pyo* py_strn(const char* s, long long len);
Pyo* py_list(Pyo** items, int n);
Pyo* py_tuple(Pyo** items, int n);
Pyo* py_dict(Pyo** keys, Pyo** vals, int n);
Pyo* py_set(Pyo** items, int n);
Pyo* py_slice(Pyo* lower, Pyo* upper, Pyo* step);

/* Functions */
Pyo* py_func(PyFn fn, const char* name, const char** params, int nparams,
             Pyo** defaults, int ndefaults, int vararg, int kwarg);
Pyo* py_closure(PyFn fn, const char* name, const char** params, int nparams,
                Pyo** defaults, int ndefaults, int vararg, int kwarg,
                Pyo** cells, int ncells);
Pyo* py_cell_new(Pyo* value);
Pyo* py_cell_get(Pyo* cell);
void py_cell_set(Pyo* cell, Pyo* value);
Pyo* py_builtin_ref(const char* name);
Pyo* py_call(Pyo* fn, Pyo** args, int argc);
Pyo* py_call_kw(Pyo* fn, Pyo** args, int argc,
                Pyo** kwnames, Pyo** kwvals, int nkw);

/* Dynamic argument vectors used for *args / **kwargs expansion */
typedef struct {
    Pyo** items;
    int len, cap;
} ArgVec;
void py_args_init(ArgVec* v);
void py_args_push(ArgVec* v, Pyo* x);
void py_args_extend(ArgVec* v, Pyo* iterable);
void py_args_extend_kv(ArgVec* names, ArgVec* vals, Pyo* dict);

/* Operators */
Pyo* py_add(Pyo* a, Pyo* b);
Pyo* py_sub(Pyo* a, Pyo* b);
Pyo* py_mul(Pyo* a, Pyo* b);
Pyo* py_div(Pyo* a, Pyo* b);
Pyo* py_floordiv(Pyo* a, Pyo* b);
Pyo* py_mod(Pyo* a, Pyo* b);
Pyo* py_pow(Pyo* a, Pyo* b);
Pyo* py_lshift(Pyo* a, Pyo* b);
Pyo* py_rshift(Pyo* a, Pyo* b);
Pyo* py_bitand(Pyo* a, Pyo* b);
Pyo* py_bitor(Pyo* a, Pyo* b);
Pyo* py_bitxor(Pyo* a, Pyo* b);
Pyo* py_neg(Pyo* a);
Pyo* py_pos(Pyo* a);
Pyo* py_invert(Pyo* a);
int  py_truth(Pyo* a);
Pyo* py_not(Pyo* a);

/* Comparisons */
Pyo* py_eq(Pyo* a, Pyo* b);
Pyo* py_ne(Pyo* a, Pyo* b);
Pyo* py_lt(Pyo* a, Pyo* b);
Pyo* py_le(Pyo* a, Pyo* b);
Pyo* py_gt(Pyo* a, Pyo* b);
Pyo* py_ge(Pyo* a, Pyo* b);
Pyo* py_in(Pyo* item, Pyo* container);
Pyo* py_is(Pyo* a, Pyo* b);
Pyo* py_isnot(Pyo* a, Pyo* b);

/* Sequences */
Pyo* py_getitem(Pyo* obj, Pyo* key);
void py_setitem(Pyo* obj, Pyo* key, Pyo* value);
Pyo* py_len(Pyo* obj);
Pyo* py_iter(Pyo* obj);
int  py_iter_next(Pyo* it, Pyo** out);

/* Methods and attributes */
Pyo* py_method(Pyo* obj, const char* name, Pyo** args, int argc);
void py_list_sort(Pyo* list, Pyo* reverse, Pyo* key);

/* Classes and instances */
Pyo* py_class(const char* name, Pyo* base);
void py_class_set(Pyo* cls, const char* name, Pyo* value);
Pyo* py_getattr(Pyo* obj, const char* name);
void py_setattr(Pyo* obj, const char* name, Pyo* value);

/* Builtins (variadic ones take an argument array) */
Pyo* py_print(Pyo** args, int argc, Pyo* sep, Pyo* end);
Pyo* py_builtin_str(Pyo* x);
Pyo* py_builtin_repr(Pyo* x);
void py_assert(int cond, Pyo* msg);
Pyo* py_builtin_int(Pyo* x);
Pyo* py_builtin_float(Pyo* x);
Pyo* py_builtin_bool(Pyo* x);
Pyo* py_abs(Pyo* x);
Pyo* py_min(Pyo** args, int argc);
Pyo* py_max(Pyo** args, int argc);
Pyo* py_sum(Pyo** args, int argc);
Pyo* py_range(Pyo** args, int argc);
Pyo* py_list_of(Pyo* x);
Pyo* py_tuple_of(Pyo* x);
Pyo* py_set_of(Pyo* x);
Pyo* py_dict_of(Pyo* x);
Pyo* py_sorted(Pyo* x, Pyo* reverse, Pyo* key);
Pyo* py_enumerate(Pyo* x, Pyo* start);
Pyo* py_zip(Pyo** args, int argc);
Pyo* py_reversed(Pyo* x);
Pyo* py_any(Pyo** args, int argc);
Pyo* py_all(Pyo** args, int argc);
Pyo* py_type(Pyo* x);
Pyo* py_isinstance(Pyo* x, Pyo* t);
Pyo* py_round(Pyo** args, int argc);
Pyo* py_pow_builtin(Pyo* a, Pyo* b);
Pyo* py_chr(Pyo* x);
Pyo* py_ord(Pyo* x);
Pyo* py_hex(Pyo* x);
Pyo* py_oct(Pyo* x);
Pyo* py_bin(Pyo* x);
Pyo* py_divmod(Pyo* a, Pyo* b);
Pyo* py_input(Pyo** args, int argc);
Pyo* py_format(Pyo* value, const char* spec, int repr);
Pyo* py_str_concat(Pyo** items, int n);
void py_list_append(Pyo* list, Pyo* v);
void py_set_add(Pyo* set, Pyo* v);
void py_dict_set(Pyo* dict, Pyo* k, Pyo* v);
Pyo* py_sys_argv(void);
void py_sys_exit(Pyo* code);
void py_set_argv(int argc, char** argv);
Pyo* py_math_call(const char* name, Pyo** args, int argc);
Pyo* py_math_const(const char* name);

/* Runtime helpers */
void py_runtime_error(const char* fmt, ...);
void py_shutdown(void);

/* Garbage collector control. py_gc_init records the stack base (pass the
   address of a local in main) and must be called before py_gc_register_root. */
void py_gc_init(void* base);
void py_gc_register_root(Pyo** slot);
void py_gc_region_push(void* base, size_t bytes);
void py_gc_region_pop(void);

#endif /* PYRT_H */
