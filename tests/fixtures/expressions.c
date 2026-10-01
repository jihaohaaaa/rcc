// tests/fixtures/expressions.c
// Integration tests for C expressions, operators, and literals.

#include <assert.h>

int main() {
    // 1. Integer Literals (Decimal, Hex, Octal, Binary)
    assert(42 == 42);
    assert(0x2A == 42);
    assert(052 == 42);
    assert(0b101010 == 42);

    // 2. Basic Arithmetic
    assert(5 + 20 - 4 == 21);
    assert(12 + 34 - 5 == 41);
    assert(5 + 6 * 7 == 47);
    assert(5 * (9 - 6) == 15);
    assert((3 + 5) / 2 == 4);
    assert(10 % 3 == 1);
    assert(-10 + 20 == 10);
    assert(- -10 == 10);
    assert(- - +10 == 10);

    // 3. Comparisons
    assert((0 == 1) == 0);
    assert((42 == 42) == 1);
    assert((0 != 1) == 1);
    assert((42 != 42) == 0);
    assert((0 < 1) == 1);
    assert((1 < 1) == 0);
    assert((2 < 1) == 0);
    assert((0 <= 1) == 1);
    assert((1 <= 1) == 1);
    assert((2 <= 1) == 0);
    assert((1 > 0) == 1);
    assert((1 > 1) == 0);
    assert((1 >= 0) == 1);
    assert((1 >= 1) == 1);
    assert((1 >= 2) == 0);

    // 4. Bitwise & Logical Operators
    assert((1 & 3) == 1);
    assert((1 | 2) == 3);
    assert((1 ^ 3) == 2);
    assert(~-2 == 1);
    assert(!0 == 1);
    assert(!1 == 0);
    assert(!2 == 0);
    assert((1 << 4) == 16);
    assert((16 >> 2) == 4);
    assert((1 && 2) == 1);
    assert((0 && 2) == 0);
    assert((1 || 0) == 1);
    assert((0 || 0) == 0);

    // 5. Ternary & Comma
    assert((1 ? 2 : 3) == 2);
    assert((0 ? 2 : 3) == 3);
    assert((1, 2, 3) == 3);

    // 6. Variables & Compound Assignments
    int a = 1;
    a += 5;
    assert(a == 6);
    a -= 2;
    assert(a == 4);
    a *= 3;
    assert(a == 12);
    a /= 4;
    assert(a == 3);
    a %= 2;
    assert(a == 1);

    // 7. Increment / Decrement
    int x = 5;
    int b = x++;
    assert(x == 6 && b == 5);
    b = ++x;
    assert(x == 7 && b == 7);
    b = x--;
    assert(x == 6 && b == 7);
    b = --x;
    assert(x == 5 && b == 5);

    return 0;
}
