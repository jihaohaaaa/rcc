// tests/fixtures/quickjs_eval_test.c
// Integration test: Compile and run QuickJS JavaScript engine via rscc.

#define CONFIG_VERSION "2024-01-13"
#define CONFIG_BIGNUM 0
#define DUMP_LEAKS 1

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

#include "vendor/quickjs/cutils.c"
#include "vendor/quickjs/libunicode.c"
#include "vendor/quickjs/libregexp.c"
#include "vendor/quickjs/dtoa.c"
#include "vendor/quickjs/quickjs.c"

int main() {
    printf("[QuickJS Test] Initializing JSRuntime...\n");
    JSRuntime *rt = JS_NewRuntime();
    assert(rt != NULL);

    printf("[QuickJS Test] Initializing JSContext...\n");
    JSContext *ctx = JS_NewContext(rt);
    assert(ctx != NULL);

    // Test 1: Simple arithmetic eval
    {
        const char *code = "1 + 2 * 3";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        int32_t res = 0;
        JS_ToInt32(ctx, &res, val);
        printf("[QuickJS Test] 1. Arithmetic: %s = %d\n", code, res);
        assert(res == 7);
        JS_FreeValue(ctx, val);
    }

    // Test 2: String concatenation and functions
    {
        const char *code = "function greet(name) { return `Hello, ${name}!`; } greet('rscc');";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        const char *str = JS_ToCString(ctx, val);
        printf("[QuickJS Test] 2. Template literal & greeting: %s\n", str);
        assert(strcmp(str, "Hello, rscc!") == 0);
        JS_FreeCString(ctx, str);
        JS_FreeValue(ctx, val);
    }

    // Test 3: Array manipulation (map, filter, reduce)
    {
        const char *code = "[1, 2, 3, 4, 5].map(x => x * x).reduce((a, b) => a + b, 0);";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        int32_t res = 0;
        JS_ToInt32(ctx, &res, val);
        printf("[QuickJS Test] 3. Array map/reduce sum: %d\n", res);
        assert(res == 55); // 1 + 4 + 9 + 16 + 25 = 55
        JS_FreeValue(ctx, val);
    }

    // Test 4: JSON parse & stringify
    {
        const char *code = "JSON.stringify(JSON.parse('{\"compiler\":\"rscc\",\"version\":1}'))";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        const char *str = JS_ToCString(ctx, val);
        printf("[QuickJS Test] 4. JSON parse/stringify: %s\n", str);
        assert(strcmp(str, "{\"compiler\":\"rscc\",\"version\":1}") == 0);
        JS_FreeCString(ctx, str);
        JS_FreeValue(ctx, val);
    }

    // Test 5: Regular expressions
    {
        const char *code = "'The quick brown fox jumps over the lazy dog'.match(/\\b\\w{4,5}\\b/g).join(', ')";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        const char *str = JS_ToCString(ctx, val);
        printf("[QuickJS Test] 5. RegExp matches: %s\n", str);
        assert(strcmp(str, "quick, brown, jumps, over, lazy") == 0);
        JS_FreeCString(ctx, str);
        JS_FreeValue(ctx, val);
    }

    // Test 6: Class syntax and inheritance
    {
        const char *code = "class Animal { constructor(name) { this.name = name; } speak() { return this.name + ' makes noise'; } }\n"
                            "class Dog extends Animal { speak() { return this.name + ' barks'; } }\n"
                            "new Dog('Rex').speak();";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        const char *str = JS_ToCString(ctx, val);
        printf("[QuickJS Test] 6. Class inheritance: %s\n", str);
        assert(strcmp(str, "Rex barks") == 0);
        JS_FreeCString(ctx, str);
        JS_FreeValue(ctx, val);
    }

    // Test 7: Exception handling (try / catch / finally)
    {
        const char *code = "let log = ''; try { log += '1'; throw new Error('boom'); } catch (e) { log += '2' + e.message; } finally { log += '3'; } log;";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        const char *str = JS_ToCString(ctx, val);
        printf("[QuickJS Test] 7. Try-catch-finally: %s\n", str);
        assert(strcmp(str, "12boom3") == 0);
        JS_FreeCString(ctx, str);
        JS_FreeValue(ctx, val);
    }

    // Test 8: Closures and lexical scoping
    {
        const char *code = "function makeCounter() { let count = 0; return () => ++count; }\n"
                            "const c = makeCounter(); c(); c(); c();";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        int32_t res = 0;
        JS_ToInt32(ctx, &res, val);
        printf("[QuickJS Test] 8. Closure counter: %d\n", res);
        assert(res == 3);
        JS_FreeValue(ctx, val);
    }

    // Test 9: Destructuring and object properties
    {
        const char *code = "(() => { const [a, b, c] = [10, 20, 30]; const obj = { a, b, c, d: 40 }; return obj.a + obj.b + obj.c + obj.d; })()";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        int32_t res = 0;
        JS_ToInt32(ctx, &res, val);
        printf("[QuickJS Test] 9. Destructuring & object properties: %d\n", res);
        assert(res == 100); // 10 + 20 + 30 + 40 = 100
        JS_FreeValue(ctx, val);
    }

    // Test 10: Set and Map collections
    {
        const char *code = "const s = new Set([1, 2, 2, 3]); const m = new Map(); m.set('size', s.size); m.get('size');";
        JSValue val = JS_Eval(ctx, code, strlen(code), "<eval>", JS_EVAL_TYPE_GLOBAL);
        assert(!JS_IsException(val));
        int32_t res = 0;
        JS_ToInt32(ctx, &res, val);
        printf("[QuickJS Test] 10. Set & Map collections: %d\n", res);
        assert(res == 3);
        JS_FreeValue(ctx, val);
    }

    JS_FreeContext(ctx);
    JS_FreeRuntime(rt);

    printf("[QuickJS Test] ALL QUICKJS EVAL TESTS PASSED!\n");
    return 0;
}
