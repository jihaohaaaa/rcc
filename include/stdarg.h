#ifndef _STDARG_H
#define _STDARG_H

#if defined(__x86_64__) && !defined(__APPLE__)
typedef struct {
    unsigned int gp_offset;
    unsigned int fp_offset;
    void *overflow_arg_area;
    void *reg_save_area;
} __va_elem;
typedef __va_elem va_list[1];

#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_end(ap) ((void)0)
#define va_copy(dest, src) ((dest)[0] = (src)[0])
#define va_arg(ap, type) \
    (*(type *)(((ap)->gp_offset < 48) \
        ? ((char *)(ap)->reg_save_area + (((ap)->gp_offset += 8) - 8)) \
        : ((char *)((ap)->overflow_arg_area = (char *)(ap)->overflow_arg_area + 8) - 8)))

#elif defined(__aarch64__) && !defined(__APPLE__)
typedef struct {
    void *__stack;
    void *__gr_top;
    void *__vr_top;
    int __gr_offs;
    int __vr_offs;
} va_list;

#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_arg(ap, type) (*(type *)((ap += ((sizeof(type) + 7) & ~7)) - ((sizeof(type) + 7) & ~7)))
#define va_end(ap) ((void)0)
#define va_copy(dest, src) ((dest) = (src))

#else
typedef char *va_list;

#define va_start(ap, last) __builtin_va_start(ap, last)
#define va_arg(ap, type) (*(type *)((ap += ((sizeof(type) + 7) & ~7)) - ((sizeof(type) + 7) & ~7)))
#define va_end(ap) ((void)0)
#define va_copy(dest, src) ((dest) = (src))
#endif

#endif /* _STDARG_H */
