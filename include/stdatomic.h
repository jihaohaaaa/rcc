#ifndef _STDATOMIC_H
#define _STDATOMIC_H

#include <stdint.h>
#include <stdbool.h>

typedef enum memory_order {
    memory_order_relaxed,
    memory_order_consume,
    memory_order_acquire,
    memory_order_release,
    memory_order_acq_rel,
    memory_order_seq_cst
} memory_order;

typedef int atomic_int;
typedef uint32_t atomic_uint;
typedef uint64_t atomic_uint64_t;

#define atomic_load(ptr) (*(ptr))
#define atomic_store(ptr, val) (*(ptr) = (val))
#define atomic_load_explicit(ptr, mo) (*(ptr))
#define atomic_store_explicit(ptr, val, mo) (*(ptr) = (val))
#define atomic_fetch_add(ptr, val) ((*(ptr)) += (val))
#define atomic_fetch_sub(ptr, val) ((*(ptr)) -= (val))
#define atomic_fetch_and(ptr, val) ((*(ptr)) &= (val))
#define atomic_fetch_or(ptr, val) ((*(ptr)) |= (val))
#define atomic_fetch_xor(ptr, val) ((*(ptr)) ^= (val))
#define atomic_fetch_add_explicit(ptr, val, mo) ((*(ptr)) += (val))
#define atomic_fetch_sub_explicit(ptr, val, mo) ((*(ptr)) -= (val))
#define atomic_fetch_and_explicit(ptr, val, mo) ((*(ptr)) &= (val))
#define atomic_fetch_or_explicit(ptr, val, mo) ((*(ptr)) |= (val))
#define atomic_fetch_xor_explicit(ptr, val, mo) ((*(ptr)) ^= (val))
#define atomic_exchange(ptr, val) (*(ptr) = (val))
#define atomic_exchange_explicit(ptr, val, mo) (*(ptr) = (val))
#define atomic_compare_exchange_strong(ptr, exp, val) (*(ptr) == *(exp) ? (*(ptr) = (val), 1) : (*(exp) = *(ptr), 0))
#define atomic_compare_exchange_weak(ptr, exp, val) atomic_compare_exchange_strong(ptr, exp, val)

#endif /* _STDATOMIC_H */
