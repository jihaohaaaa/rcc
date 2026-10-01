// tests/fixtures/stdlib.c
// Integration tests for standard C headers and libc runtime functions.

#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

int main() {
    // 1. <stdbool.h>
    bool flag = true;
    assert(flag);
    flag = false;
    assert(!flag);

    // 2. <stdint.h> & <stddef.h>
    uint8_t u8 = 250;
    u8 += 10;
    assert(u8 == 4); // 260 % 256 = 4

    int32_t i32 = INT32_MAX;
    assert(i32 > 0);

    uint64_t u64 = 1000000000000ULL;
    assert(u64 / 1000000000ULL == 1000);

    size_t sz = sizeof(uint64_t);
    assert(sz == 8);

    // 3. <string.h>
    char s1[32] = "Hello, ";
    char s2[16] = "World!";
    strcat(s1, s2);
    assert(strlen(s1) == 13);
    assert(strcmp(s1, "Hello, World!") == 0);

    char buf[32];
    memset(buf, 0, 32);
    memcpy(buf, "test_data", 9);
    assert(memcmp(buf, "test_data", 9) == 0);
    assert(buf[9] == 0);

    // 4. <stdio.h>
    char fmt_buf[64];
    int n = sprintf(fmt_buf, "Answer: %d, Str: %s", 42, "ok");
    assert(n == 19);
    assert(fmt_buf[0] == 'A' && fmt_buf[8] == '4' && fmt_buf[9] == '2');

    // 5. <stdlib.h>
    int *dyn = (int *)malloc(10 * sizeof(int));
    assert(dyn != NULL);
    for (int i = 0; i < 10; i++) {
        dyn[i] = i * i;
    }
    assert(dyn[5] == 25);
    assert(dyn[9] == 81);
    free(dyn);

    assert(abs(-42) == 42);
    assert(atoi("12345") == 12345);

    // 6. <ctype.h>
    assert(isalpha('A'));
    assert(isalpha('z'));
    assert(!isalpha('9'));
    assert(isdigit('7'));
    assert(!isdigit('x'));
    assert(toupper('g') == 'G');
    assert(tolower('T') == 't');

    // 7. <errno.h> & <time.h>
    errno = 0;
    assert(errno == 0);
    errno = EINVAL;
    assert(errno == 22);

    time_t now = time(NULL);
    assert(now > 0);

    return 0;
}
