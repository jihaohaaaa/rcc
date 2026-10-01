// tests/fixtures/preprocessor.c
// Integration tests for C preprocessor: macros, stringification, token-pasting, conditionals.

#include <assert.h>
#include <string.h>

// 1. Object-like macros
#define CONST_VAL 100
#define NESTED_VAL (CONST_VAL + 50)

// 2. Function-like macros
#define SQUARE(x) ((x) * (x))
#define MAX(a, b) ((a) > (b) ? (a) : (b))

// 3. Stringification
#define TO_STR(x) #x

// 4. Token-pasting
#define GLUE(a, b) a##b

// 5. Conditionals
#define FEATURE_ENABLED 1

#ifdef FEATURE_ENABLED
#define FEATURE_CODE 200
#else
#define FEATURE_CODE 400
#endif

#ifndef NOT_DEFINED
#define SAFE_VAL 77
#else
#define SAFE_VAL 0
#endif

#if CONST_VAL >= 50
#define LEVEL 1
#else
#define LEVEL 0
#endif

int main() {
    // 1. Object-like macros
    assert(CONST_VAL == 100);
    assert(NESTED_VAL == 150);

    // 2. Function-like macros
    assert(SQUARE(5) == 25);
    assert(SQUARE(3 + 2) == 25); // ((3 + 2) * (3 + 2)) == 25
    assert(MAX(10, 20) == 20);
    assert(MAX(30, 5) == 30);

    // 3. Stringification
    assert(strcmp(TO_STR(hello_world), "hello_world") == 0);
    assert(strcmp(TO_STR(123 + 456), "123 + 456") == 0);

    // 4. Token-pasting
    int GLUE(my_, var) = 999;
    assert(my_var == 999);

    // 5. Conditionals
    assert(FEATURE_CODE == 200);
    assert(SAFE_VAL == 77);
    assert(LEVEL == 1);

    // 6. Predefined macros
    assert(__LINE__ > 0);

    return 0;
}
