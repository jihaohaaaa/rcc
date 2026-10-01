// qjs_unity.c: Standalone QuickJS interpreter compiled with rscc
#define CONFIG_VERSION "2024-01-13"
#define CONFIG_BIGNUM 0

#include "vendor/quickjs/cutils.c"
#include "vendor/quickjs/libunicode.c"
#include "vendor/quickjs/libregexp.c"
#include "vendor/quickjs/dtoa.c"
#include "vendor/quickjs/quickjs.c"

#undef malloc
#undef free
#undef realloc

#include "vendor/quickjs/quickjs-libc.c"

#include "vendor/quickjs/repl.c"
#include "vendor/quickjs/qjs.c"
