// tests/fixtures/quickjs_regexp_test.c
// Test compiling and executing QuickJS libunicode.c and libregexp.c

#include <assert.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>

#include "vendor/quickjs/cutils.h"
#include "vendor/quickjs/libunicode.h"
#include "vendor/quickjs/libregexp.h"

// Include implementations for single-compilation-unit testing
#include "vendor/quickjs/cutils.c"
#include "vendor/quickjs/libunicode.c"
#include "vendor/quickjs/libregexp.c"

// Required callbacks for libregexp
int lre_check_stack_overflow(void *opaque, size_t alloca_size) {
    return 0;
}

int lre_check_timeout(void *opaque) {
    return 0;
}

void *lre_realloc(void *opaque, void *ptr, size_t size) {
    return realloc(ptr, size);
}

int main() {
    // 1. Test libunicode character classification & case conversion
    assert(lre_is_space(' ') == 1);
    assert(lre_is_space('\t') == 1);
    assert(lre_is_space('A') == 0);

    assert(lre_is_id_start('a') == 1);
    assert(lre_is_id_start('9') == 0);

    assert(lre_is_id_continue('a') == 1);
    assert(lre_is_id_continue('9') == 1);
    assert(lre_is_id_continue('-') == 0);

    // 2. Test regex compilation and matching: simple pattern
    const char *pattern = "[0-9]+";
    int bc_len = 0;
    char error_msg[128];
    uint8_t *bc = lre_compile(&bc_len, error_msg, sizeof(error_msg),
                              pattern, strlen(pattern), 0, NULL);
    assert(bc != NULL);
    assert(bc_len > 0);

    const char *input = "abc12345xyz";
    int capture_count = lre_get_capture_count(bc);
    assert(capture_count >= 1);

    uint8_t **capture = (uint8_t **)malloc(sizeof(uint8_t *) * capture_count * 2);
    int res = lre_exec(capture, bc, (const uint8_t *)input, 0, strlen(input), 0, NULL);
    assert(res == 1); // Match success

    // Match start should point to '1', match end should point to 'x'
    assert(capture[0] == (const uint8_t *)input + 3);
    assert(capture[1] == (const uint8_t *)input + 8);

    free(capture);
    free(bc);

    // 3. Test regex with capture group: (\w+)@(\w+)
    const char *email_pat = "([a-z]+)@([a-z]+)";
    bc = lre_compile(&bc_len, error_msg, sizeof(error_msg),
                     email_pat, strlen(email_pat), 0, NULL);
    assert(bc != NULL);

    capture_count = lre_get_capture_count(bc);
    assert(capture_count == 3); // Full match + 2 sub-groups

    capture = (uint8_t **)malloc(sizeof(uint8_t *) * capture_count * 2);
    const char *email_str = "contact user@domain now";
    res = lre_exec(capture, bc, (const uint8_t *)email_str, 0, strlen(email_str), 0, NULL);
    assert(res == 1);

    // Group 1: "user"
    assert(capture[2] == (const uint8_t *)email_str + 8);
    assert(capture[3] == (const uint8_t *)email_str + 12);

    // Group 2: "domain"
    assert(capture[4] == (const uint8_t *)email_str + 13);
    assert(capture[5] == (const uint8_t *)email_str + 19);

    free(capture);
    free(bc);

    return 0;
}
