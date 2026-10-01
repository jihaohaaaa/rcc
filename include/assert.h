#ifndef _ASSERT_H
#define _ASSERT_H

#ifdef NDEBUG
#define assert(ignore) ((void)0)
#else
void abort(void);
int printf(const char *format, ...);
#define assert(expr) ((expr) ? (void)0 : (printf("Assertion failed: %s, file %s, line %d\n", #expr, __FILE__, __LINE__), abort()))
#endif

#endif /* _ASSERT_H */

