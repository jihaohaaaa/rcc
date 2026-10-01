// tests/fixtures/quickjs_cutils_test.c
// Test compiling and running QuickJS cutils.c / cutils.h

#include <assert.h>
#include <string.h>
#include <stdint.h>
#include <stdlib.h>

#include "vendor/quickjs/cutils.h"
#include "vendor/quickjs/cutils.c"

int test_cmp(const void *a, const void *b, void *opaque) {
    int va = *(const int *)a;
    int vb = *(const int *)b;
    return (va > vb) - (va < vb);
}

int main() {
    // 1. String utilities: pstrcpy, pstrcat, strstart, has_suffix
    char buf[32];
    pstrcpy(buf, sizeof(buf), "hello");
    assert(strcmp(buf, "hello") == 0);
    pstrcat(buf, sizeof(buf), " world!");
    assert(strcmp(buf, "hello world!") == 0);

    const char *rest = NULL;
    assert(strstart("foobar", "foo", &rest) == 1);
    assert(strcmp(rest, "bar") == 0);
    assert(strstart("foobar", "baz", &rest) == 0);

    assert(has_suffix("filename.js", ".js") == 1);
    assert(has_suffix("filename.js", ".c") == 0);

    // 2. Math & Bit utilities: clz, ctz, bswap, min/max
    assert(max_int(10, 20) == 20);
    assert(min_int(10, 20) == 10);
    assert(clz32(1) == 31);
    assert(clz32(0x80000000) == 0);
    assert(ctz32(8) == 3);
    assert(bswap16(0x1234) == 0x3412);
    assert(bswap32(0x12345678) == 0x78563412);

    // 3. DynBuf dynamic buffer operations
    DynBuf db;
    dbuf_init(&db);
    dbuf_putstr(&db, "QuickJS");
    dbuf_putc(&db, ' ');
    dbuf_putstr(&db, "Engine");
    assert(db.size == 14);
    assert(memcmp(db.buf, "QuickJS Engine", 14) == 0);

    dbuf_put_u16(&db, 0x1234);
    dbuf_put_u32(&db, 0xAABBCCDD);
    assert(get_u16(db.buf + 14) == 0x1234);
    assert(get_u32(db.buf + 16) == 0xAABBCCDD);

    dbuf_free(&db);

    // 4. Unicode UTF-8 decoding / encoding
    uint8_t utf8_buf[8];
    int len = unicode_to_utf8(utf8_buf, 0x4E2D); // '中' in UTF-8: E4 B8 AD
    assert(len == 3);
    assert(utf8_buf[0] == 0xE4 && utf8_buf[1] == 0xB8 && utf8_buf[2] == 0xAD);

    const uint8_t *p = utf8_buf;
    int code = unicode_from_utf8(p, 3, &p);
    assert(code == 0x4E2D);

    // 5. rqsort sorting
    int arr[5] = {50, 10, 40, 20, 30};
    rqsort(arr, 5, sizeof(int), test_cmp, NULL);
    assert(arr[0] == 10 && arr[1] == 20 && arr[2] == 30 && arr[3] == 40 && arr[4] == 50);

    return 0;
}
