// tests/fixtures/control_flow.c
// Integration tests for C control flow statements: if/else, while, do-while, for, switch, goto.

#include <assert.h>

int main() {
    // 1. If-Else
    int v = 0;
    if (1) {
        v = 10;
    } else {
        v = 20;
    }
    assert(v == 10);

    if (0) {
        v = 10;
    } else {
        v = 20;
    }
    assert(v == 20);

    // 2. While Loop
    int sum = 0;
    int i = 1;
    while (i <= 10) {
        sum += i;
        i += 1;
    }
    assert(sum == 55);
    assert(i == 11);

    // 3. Do-While Loop
    int d = 0;
    do {
        d += 1;
    } while (d < 5);
    assert(d == 5);

    int d2 = 10;
    do {
        d2 += 1;
    } while (d2 < 5);
    assert(d2 == 11);

    // 4. For Loop
    int for_sum = 0;
    for (int k = 0; k <= 10; k++) {
        for_sum += k;
    }
    assert(for_sum == 55);

    // Nested for loop
    int grid_count = 0;
    for (int r = 0; r < 5; r++) {
        for (int c = 0; c < 5; c++) {
            grid_count += 1;
        }
    }
    assert(grid_count == 25);

    // 5. Break and Continue
    int brk_sum = 0;
    for (int n = 0; n < 10; n++) {
        if (n == 5) break;
        brk_sum += n;
    }
    assert(brk_sum == 10); // 0 + 1 + 2 + 3 + 4 = 10

    int cont_sum = 0;
    for (int n = 0; n < 10; n++) {
        if (n % 2 == 1) continue;
        cont_sum += n;
    }
    assert(cont_sum == 20); // 0 + 2 + 4 + 6 + 8 = 20

    // 6. Switch-Case
    int target = 2;
    int res = 0;
    switch (target) {
        case 1:
            res = 10;
            break;
        case 2:
            res = 20;
            break;
        default:
            res = 30;
            break;
    }
    assert(res == 20);

    target = 99;
    switch (target) {
        case 1:
            res = 10;
            break;
        default:
            res = 42;
            break;
    }
    assert(res == 42);

    // 7. Goto and Labels
    int g_val = 0;
    goto jump_target;
    g_val = 100;

jump_target:
    assert(g_val == 0);

    int loop_cnt = 0;
loop_head:
    loop_cnt++;
    if (loop_cnt < 7) {
        goto loop_head;
    }
    assert(loop_cnt == 7);

    return 0;
}
