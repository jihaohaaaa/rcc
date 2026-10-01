// tests/fixtures/pointers.c
// Integration tests for pointers, dereferencing, pointer arithmetic, and arrays.

#include <assert.h>

int main() {
    // 1. Address-of and Dereference
    int x = 3;
    assert(*&x == 3);

    int *y = &x;
    *y = 5;
    assert(x == 5);

    int a_val = 3;
    int b_val = 5;
    int *p1 = &a_val;
    int *p2 = &b_val;
    *p1 = *p2;
    assert(a_val == 5);

    // 2. Multilevel Pointers
    int val = 42;
    int *p = &val;
    int **pp = &p;
    assert(**pp == 42);
    **pp = 100;
    assert(val == 100);

    // 3. Pointer Arithmetic
    int arr[5];
    for (int i = 0; i < 5; i++) {
        *(arr + i) = (i + 1) * 10;
    }
    assert(arr[0] == 10);
    assert(arr[1] == 20);
    assert(arr[2] == 30);
    assert(arr[3] == 40);
    assert(arr[4] == 50);

    int *start = &arr[1];
    int *end = &arr[4];
    assert(end - start == 3);
    assert(*(start + 2) == 40);

    // 4. 2D Arrays
    int mat[2][3];
    mat[0][0] = 1; mat[0][1] = 2; mat[0][2] = 3;
    mat[1][0] = 4; mat[1][1] = 5; mat[1][2] = 6;

    assert(mat[0][0] == 1);
    assert(mat[1][2] == 6);
    assert(*(*(mat + 1) + 1) == 5);

    // 5. Array Initializers
    int init_arr[4] = {10, 20, 30, 40};
    assert(init_arr[0] == 10);
    assert(init_arr[1] == 20);
    assert(init_arr[2] == 30);
    assert(init_arr[3] == 40);

    return 0;
}
