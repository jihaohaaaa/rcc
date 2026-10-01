// tests/fixtures/functions.c
// Integration tests for function declarations, definitions, parameter passing, and recursion.

#include <assert.h>

// Forward declarations
int add(int a, int b);
int add8(int a, int b, int c, int d, int e, int f, int g, int h);
int fact(int n);
int fib(int n);
int is_even(int n);
int is_odd(int n);

int add(int a, int b) {
    return a + b;
}

int add8(int a, int b, int c, int d, int e, int f, int g, int h) {
    return a + b + c + d + e + f + g + h;
}

int fact(int n) {
    if (n <= 1) return 1;
    return n * fact(n - 1);
}

int fib(int n) {
    if (n <= 0) return 0;
    if (n == 1) return 1;
    return fib(n - 1) + fib(n - 2);
}

// Mutual recursion
int is_even(int n) {
    if (n == 0) return 1;
    return is_odd(n - 1);
}

int is_odd(int n) {
    if (n == 0) return 0;
    return is_even(n - 1);
}

int main() {
    // 1. Basic Function Call
    assert(add(15, 27) == 42);

    // 2. Many Arguments (Register passing AAPCS64 x0-x7)
    assert(add8(1, 2, 3, 4, 5, 6, 7, 8) == 36);

    // 3. Direct Recursion
    assert(fact(5) == 120);
    assert(fib(9) == 34);
    assert(fib(10) == 55);

    // 4. Mutual Recursion
    assert(is_even(10) == 1);
    assert(is_odd(10) == 0);
    assert(is_even(7) == 0);
    assert(is_odd(7) == 1);

    return 0;
}
